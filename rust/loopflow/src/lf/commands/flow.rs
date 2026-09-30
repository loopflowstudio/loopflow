use crate::durable::{FlowSession, TaskFlowBlocker, TaskWorkerClaim, WorkRef};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::{
    expand_flow, human_occurrence_ids, ConcreteSkill, ConcreteStep, ConcreteXor, ExecutionContext,
    ExecutionCursor, Flow, FlowEngine, FlowOutcome, SkillExecutor, SkillOutcome, StepProgress,
};
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::Cli;
use crate::ops::{flow_run, NullProgress, WorkBinding};
use crate::store::SharedStore;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::Path;
use std::sync::Mutex;

/// Run a flow: print pipeline header, then execute each skill sequentially.
pub fn run(
    flow: &Flow,
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
    binding: Option<&WorkBinding>,
) -> Result<()> {
    let items = expand_flow(flow, repo)?;
    print_pipeline_header(&flow.name, &items);
    let bound_message =
        binding.map(|binding| crate::lf::commands::run::bound_message(binding, message));
    execute(
        &flow.name,
        &items,
        bound_message.as_deref().or(message),
        cli,
        repo,
        binding,
    )
}

pub fn show(name: &str, repo: &Path) -> Result<()> {
    let flow = crate::engine::load_flow(name, repo)?;
    let items = expand_flow(&flow, repo)?;
    for line in render_pipeline_lines(&items) {
        println!("{line}");
    }
    Ok(())
}

/// `lf flow list [--json]` — selectable Flows with the topology each would pin.
pub fn list(repo: &Path, json: bool) -> Result<()> {
    let catalog = crate::engine::flow_graph::flow_catalog(repo);
    if json {
        println!("{}", serde_json::to_string(&catalog)?);
        return Ok(());
    }
    for entry in catalog {
        match entry.unavailable {
            Some(reason) => println!("{}  (unavailable: {reason})", entry.name),
            None => println!("{}", entry.name),
        }
    }
    Ok(())
}

pub fn validate(name: &str, repo: &Path) -> Result<()> {
    let flow = crate::engine::load_flow(name, repo)?;
    let mut human = human_occurrence_ids(&flow, repo)?;
    human.sort();
    if human.is_empty() {
        println!("{}: valid (no review steps)", flow.name);
    } else {
        println!("{}: valid (review steps: {})", flow.name, human.join(", "));
    }
    Ok(())
}

/// Store the invocation row and drive it, bracketed by flow journal events.
/// A Flow whose row cannot be written does not start.
fn execute(
    flow_name: &str,
    items: &[ConcreteStep],
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
    binding: Option<&WorkBinding>,
) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let flow = FlowSession {
        parent_id: None,
        invocation: QueuedInvocation::new(flow_name, items.to_vec())?,
        cursor: ExecutionCursor::default(),
        version: 0,
        task_id: binding.and_then(|binding| match &binding.work {
            WorkRef::Task(id) => Some(id.clone()),
            _ => None,
        }),
        wave_id: binding.map(|binding| binding.wave_id.clone()),
        cwd: repo.to_path_buf(),
        message: message.map(str::to_owned),
        model: cli.model.clone(),
        current_attempt: None,
        pending_session_id: None,
        ready_summary: None,
        worker_generation: 0,
        claim: None,
        failure: None,
        finished: false,
        updated_at: time::OffsetDateTime::now_utc(),
    };
    let (store, flow) = runtime.block_on(async {
        let store = open_flow_store().await?;
        let flow = store.create_flow(flow).await?;
        Ok::<_, anyhow::Error>((store, flow))
    })?;
    eprintln!(
        "Flow invocation {} — resume with lf flow resume {}",
        flow.id(),
        flow.id()
    );
    report_outcome(runtime.block_on(drive(store, flow, None, cli))?)
}

async fn open_flow_store() -> Result<SharedStore> {
    let config = crate::store::storage_config_from_env()?;
    Ok(std::sync::Arc::new(
        crate::store::open_store(&config).await?,
    ))
}

fn report_outcome(outcome: FlowOutcome) -> Result<()> {
    match outcome {
        FlowOutcome::Completed => Ok(()),
        FlowOutcome::Waiting => {
            anyhow::bail!("Flow is waiting for human input; open its Flow Session")
        }
        FlowOutcome::Blocked(reason) => anyhow::bail!("Flow is blocked: {reason}"),
    }
}

pub fn control(command: &str, args: &[String], cli: &Cli) -> Result<()> {
    match command {
        "blocked" => {
            let reason = args.join(" ");
            anyhow::ensure!(!reason.trim().is_empty(), "usage: lf flow blocked REASON");
            let actor =
                journal::current_exec_id().ok_or_else(|| anyhow!("command Exec is unavailable"))?;
            let step = active_step()?;
            block_on_store(|store| async move {
                let key = store
                    .flow_blocker_key(&step.invocation, step.version, &actor)
                    .await?;
                let summary =
                    crate::ops::human_session::ask_once(&store, &key, &reason, Some("unblock"))
                        .await?;
                println!("Session complete: {summary}\nReassess the current evidence before choosing Advance or Iterate. This returned feedback, not a navigation decision.");
                Ok(())
            })
        }
        "resume" => {
            anyhow::ensure!(
                args.len() == 1 || (args.len() == 2 && args[1] == "--retry"),
                "usage: lf flow resume INVOCATION [--retry]"
            );
            let id = args[0].as_str();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let store = runtime.block_on(open_flow_store())?;
            let flow = runtime
                .block_on(store.flow(id))?
                .ok_or_else(|| anyhow!("Flow {id} has no invocation row"))?;
            let flow = if flow.finished {
                flow
            } else {
                store.sqlite.active_flow(id)?
            };
            let id = flow.id().to_owned();
            let id = id.as_str();
            if let Some(task_id) = &flow.task_id {
                if runtime
                    .block_on(store.task_flow(task_id))?
                    .is_some_and(|managed| managed.id() == id)
                {
                    // The Task owns launch policy, agent choice and unblock feedback.
                    // Its worker enters the same driver under the existing claim path.
                    let task = runtime
                        .block_on(store.get_task(task_id))?
                        .ok_or_else(|| anyhow!("Task {task_id} is missing"))?;
                    crate::ops::task::task_run(
                        &task.worktree,
                        &task.plan.identifier,
                        crate::ops::task::TaskLaunchOptions {
                            retry: args.len() == 2,
                            agent: cli.model.clone(),
                            ..Default::default()
                        },
                    )?;
                    return Ok(());
                }
            }
            let flow = if args.len() == 2 {
                runtime.block_on(prepare_native_retry(&store, flow))?
            } else {
                flow
            };
            let flow = if args.len() == 2 && flow.failure.is_some() {
                let _driver = flow_run::driver_lock(&store.sqlite.flow_root(id)?)?;
                runtime.block_on(store.retry_flow(id, None))?
            } else {
                flow
            };
            let argv = std::env::args().collect::<Vec<_>>();
            let cwd = flow.cwd.clone();
            journal::with_runtime(&cwd, &argv, || {
                report_outcome(runtime.block_on(drive(store, flow, None, cli))?)
            })
        }
        _ => anyhow::bail!("unknown Flow control {command}"),
    }
}

fn block_on_store<F, Fut>(operation: F) -> Result<()>
where
    F: FnOnce(SharedStore) -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = std::sync::Arc::new(
            crate::store::open_existing_store()
                .await
                .ok_or_else(|| anyhow!("no Loopflow registry on this Home"))?,
        );
        operation(store).await
    })
}

fn active_step() -> Result<flow_run::ActiveStep> {
    flow_run::token()?.ok_or_else(|| anyhow!("this operation requires a Flow step"))
}

/// An operator-requested stop releases the position without recording a failure.
#[derive(Debug)]
pub(crate) enum StepEnd {
    Interrupted,
}

impl std::fmt::Display for StepEnd {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interrupted => formatter.write_str("step interrupted"),
        }
    }
}

impl std::error::Error for StepEnd {}

/// Prepare uncertain native work for an explicit retry. Both Task launch and
/// taskless resume use this before acquiring a replacement worker claim. Recorded
/// failures remain with the caller's existing retry/unblock policy.
pub(crate) async fn prepare_native_retry(
    store: &SharedStore,
    flow: FlowSession,
) -> Result<FlowSession> {
    wait_for_step(store, &flow).await?;
    if store.sqlite.pending_flow_conversation(flow.id())?.is_none() {
        return Ok(flow);
    }
    let _driver = flow_run::driver_lock(&store.sqlite.flow_root(flow.id())?)?;
    let saved = store
        .flow(flow.id())
        .await?
        .ok_or_else(|| anyhow!("Flow disappeared"))?;
    anyhow::ensure!(
        saved.version == flow.version && saved.claim == flow.claim,
        "Flow changed before native retry"
    );
    recover_native_flow(store, flow.id(), flow.claim.as_ref(), true).await
}

/// Preserve a surviving step's write authority until its own result is recorded.
pub(crate) async fn wait_for_step(store: &SharedStore, flow: &FlowSession) -> Result<()> {
    loop {
        let current = store.flow(flow.id()).await?.context("Flow disappeared")?;
        anyhow::ensure!(
            current.version == flow.version && current.claim == flow.claim,
            "Flow changed while observing its step"
        );
        let Some(exec) = store.sqlite.pending_flow_step_exec(flow.id())? else {
            return Ok(());
        };
        // In-process historical captures and managed Task steps still name their
        // driver. Waiting for ourselves would prevent that driver from settling.
        if journal::current_exec_id().as_ref() == Some(&exec) {
            return Ok(());
        }
        match journal::exec_process_evidence(&store.sqlite, &exec) {
            journal::ProcessIdentityEvidence::Live => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            journal::ProcessIdentityEvidence::Dead => return Ok(()),
            journal::ProcessIdentityEvidence::Unknown => {
                anyhow::bail!("Flow {} step Exec {exec} has unknown process identity; retain its pending effect", flow.id());
            }
        }
    }
}

/// Read the selected provider turn before judging the Flow. A surviving engine
/// can finish after its driver exits; observing that result does not repair the
/// driver's unknown command outcome or grant a new conversation driver claim.
async fn recover_native_flow(
    store: &SharedStore,
    id: &str,
    claim: Option<&TaskWorkerClaim>,
    retry: bool,
) -> Result<FlowSession> {
    loop {
        let flow = store
            .flow(id)
            .await?
            .ok_or_else(|| anyhow!("Flow {id} disappeared"))?;
        anyhow::ensure!(
            flow.claim.as_ref() == claim,
            "Flow {id} changed under its driver"
        );
        wait_for_step(store, &flow).await?;
        if flow
            .current_attempt
            .as_ref()
            .is_some_and(|attempt| !attempt.published)
        {
            if let Some(exec) = store.sqlite.pending_flow_step_exec(id)? {
                if journal::exec_process_evidence(&store.sqlite, &exec)
                    == journal::ProcessIdentityEvidence::Dead
                {
                    // No provider may start before publication. Retain this capture
                    // in history and let the next command capture its own input.
                    return Ok(store.release_flow(id, flow.version, claim).await?);
                }
            }
        }
        let Some(session_id) = store.sqlite.pending_flow_conversation(id)? else {
            break;
        };
        if retry && crate::run_record::conversation_engine_exited(&store.sqlite, &session_id)? {
            // Missing native completion remains unknown. Explicit retry releases
            // only the fenced boundary after exact engine exit evidence.
            return Ok(store.release_flow(id, flow.version, claim).await?);
        }
        let (endpoint, thread_id) =
            store
                .sqlite
                .session_connection(&session_id)?
                .ok_or_else(|| {
                    anyhow!(
                        "Selected conversation {session_id} has no native connection for recovery"
                    )
                })?;
        let session = store
            .sqlite
            .session(&session_id)?
            .context("Selected Session is missing")?;
        match session.provider.as_deref() {
            Some("opencode") => {
                crate::harness::opencode_history::recover(
                    &store.sqlite,
                    &session_id,
                    &endpoint,
                    &thread_id,
                )
                .await?
            }
            Some("codex") => {
                let connection = crate::harness::codex_connection::CodexConnection {
                    store: store.sqlite.clone(),
                    session_id,
                    thread_id,
                    driver: None,
                };
                tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    connection.recover_history(Path::new(&endpoint)),
                )
                .await
                .context("Selected native turn history did not respond")??;
            }
            provider => {
                anyhow::bail!("Selected Session provider {provider:?} has no native history reader")
            }
        }
        if store.sqlite.pending_flow_conversation(id)?.is_some() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    Ok(store.recover_flow(id, claim).await?)
}

/// Drive one invocation from its row until it completes, waits or blocks.
/// `claim` is the Task worker's, held until the driver stops; every write is
/// fenced by it. The engine owns traversal; child commands read their work,
/// model fallback and captured definition from the selected Flow row.
pub(crate) async fn drive(
    store: SharedStore,
    flow: FlowSession,
    claim: Option<TaskWorkerClaim>,
    launcher: &Cli,
) -> Result<FlowOutcome> {
    let fields = |extra: LfEventFields| LfEventFields {
        flow: Some(flow.invocation.flow.clone()),
        ..extra
    };
    journal::emit(
        &flow.cwd,
        LfNode::Flow,
        LfEventType::Started,
        fields(LfEventFields::default()),
    );
    let result = drive_loop(store, &flow, claim, launcher).await;
    let (event, error) = match &result {
        Ok(FlowOutcome::Completed) => (LfEventType::Completed, None),
        Ok(_) => (LfEventType::Escalated, None),
        Err(error) => (LfEventType::Errored, Some(error.to_string())),
    };
    journal::emit(
        &flow.cwd,
        LfNode::Flow,
        event,
        fields(LfEventFields {
            error,
            ..Default::default()
        }),
    );
    result
}

async fn drive_loop(
    store: SharedStore,
    flow: &FlowSession,
    claim: Option<TaskWorkerClaim>,
    launcher: &Cli,
) -> Result<FlowOutcome> {
    let root = store.sqlite.flow_root(flow.id())?;
    let _driver = flow_run::driver_lock(&root)?;
    let _flow_env = EnvVarGuard::set("LOOPFLOW_FLOW_NAME", &flow.invocation.flow);
    let mut owned_claim = claim;
    loop {
        let selected = store.sqlite.active_flow(&root)?;
        let id = selected.id().to_owned();
        let mut flow = recover_native_flow(&store, &id, owned_claim.as_ref(), false).await?;
        if flow.finished {
            return Ok(FlowOutcome::Completed);
        }
        if let Some(failure) = &flow.failure {
            anyhow::bail!(
                "Flow {id} is blocked: {}; resume with `lf flow resume {root} --retry`",
                failure.reason
            );
        }
        let executor = CliFlowExecutor {
            store: store.clone(),
            id: id.clone(),
            version: Mutex::new(flow.version),
            claim: Mutex::new(owned_claim.clone()),
            progress: Mutex::new(None),
            launcher,
        };
        let steps = &flow.invocation.steps;
        let outcome = if flow.cursor.index == steps.len() {
            Ok(Some(FlowOutcome::Completed))
        } else {
            match FlowEngine::new(&executor)
                .tick(steps, &mut flow.cursor)
                .await
            {
                Ok(outcome) => (&executor).checkpoint(&flow.cursor).await.map(|()| outcome),
                Err(error) => Err(error),
            }
        };
        let version = executor.version();
        let claim = executor.claim();
        let progress = executor
            .progress
            .lock()
            .expect("Flow progress mutex poisoned")
            .take();
        return match outcome {
            Ok(None) => {
                owned_claim = claim;
                continue;
            }
            Ok(Some(FlowOutcome::Completed)) => {
                store
                    .end_flow(
                        &id,
                        version,
                        claim.as_ref(),
                        progress.as_deref().unwrap_or_default(),
                    )
                    .await?;
                Ok(FlowOutcome::Completed)
            }
            Ok(Some(FlowOutcome::Waiting)) => Ok(FlowOutcome::Waiting),
            Ok(Some(FlowOutcome::Blocked(reason))) => {
                store
                    .fail_flow(&id, version, claim.as_ref(), &TaskFlowBlocker::now(&reason))
                    .await?;
                anyhow::bail!(
                    "Flow {id} blocked: {reason}; resume with `lf flow resume {id} --retry`"
                )
            }
            // A step's failure is what the caller hears; a write the row no longer
            // accepts (the Flow was replaced or ended meanwhile) is logged.
            Err(error) => match error.downcast_ref::<StepEnd>() {
                Some(StepEnd::Interrupted) => {
                    record(store.release_flow(&id, version, claim.as_ref()).await);
                    Ok(FlowOutcome::Waiting)
                }
                None => {
                    record(
                        store
                            .fail_flow(
                                &id,
                                version,
                                claim.as_ref(),
                                &TaskFlowBlocker::now(format!("{error:#}")),
                            )
                            .await,
                    );
                    anyhow::bail!(
                        "Flow {id} blocked: {error:#}; resume with `lf flow resume {id} --retry`"
                    )
                }
            },
        };
    }
}

fn record<T>(written: crate::store::StoreResult<T>) {
    if let Err(error) = written {
        tracing::warn!(%error, "Flow failure was not recorded on its row");
    }
}

fn print_pipeline_header(flow_name: &str, items: &[ConcreteStep]) {
    let colors = Colors::new();
    let lines = render_pipeline_lines(items);
    let pipeline = lines
        .into_iter()
        .map(|line| {
            format!(
                "  {dim}{line}{reset}",
                dim = colors.dim,
                reset = colors.reset
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    eprintln!(
        "\n{dim}\u{2500}\u{2500} flow {reset}{bold}{name}{reset}\n{pipeline}\n",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = flow_name,
        pipeline = pipeline,
    );
}

fn render_pipeline_lines(items: &[ConcreteStep]) -> Vec<String> {
    items.iter().flat_map(render_pipeline_item).collect()
}

fn render_pipeline_item(item: &ConcreteStep) -> Vec<String> {
    match item {
        ConcreteStep::Skill(skill) if skill.policy.human => vec![format!(
            "{} [review:{}]",
            skill.skill.name,
            skill
                .policy
                .id
                .as_deref()
                .expect("validated review node has an id"),
        )],
        ConcreteStep::Skill(skill) => vec![skill.skill.name.clone()],
        ConcreteStep::Op(ops) => vec![format!("op: {}", ops.item.display_name())],
        ConcreteStep::Xor(branch) => render_branch_pipeline(branch),
    }
}

fn render_branch_pipeline(branch: &ConcreteXor) -> Vec<String> {
    let mut lines = vec![format!("[xor via {}]", branch.router.name)];
    let mut paths: Vec<_> = branch.paths.iter().collect();
    paths.sort_by_key(|(name, _)| *name);

    for (index, (key, path)) in paths.iter().enumerate() {
        let nested = render_pipeline_lines(&path.steps);
        let branch_prefix = tree_prefix(index, paths.len());
        if nested.is_empty() {
            lines.push(format!("{branch_prefix} {key}"));
            continue;
        }

        let nested_chain = nested.join(" → ");
        lines.push(format!("{branch_prefix} {key} → {nested_chain}"));
    }

    lines
}

fn tree_prefix(index: usize, total: usize) -> &'static str {
    if index + 1 == total {
        "└─"
    } else {
        "├─"
    }
}

pub(crate) struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    pub(crate) fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

/// The one Flow executor: every step reads and writes the invocation row.
struct CliFlowExecutor<'a> {
    store: SharedStore,
    id: String,
    version: Mutex<u64>,
    /// The Task worker's claim while the driver holds the position.
    claim: Mutex<Option<TaskWorkerClaim>>,
    /// The finished step's summary, reported with the next checkpoint.
    progress: Mutex<Option<String>>,
    launcher: &'a Cli,
}

impl CliFlowExecutor<'_> {
    fn version(&self) -> u64 {
        *self.version.lock().expect("Flow version mutex poisoned")
    }

    fn claim(&self) -> Option<TaskWorkerClaim> {
        self.claim
            .lock()
            .expect("Flow claim mutex poisoned")
            .clone()
    }

    fn observe(&self, flow: &FlowSession) {
        *self.version.lock().expect("Flow version mutex poisoned") = flow.version;
        if flow.claim.is_none() {
            *self.claim.lock().expect("Flow claim mutex poisoned") = None;
        }
    }

    /// The row at the step about to run, with any earlier attempt settled.
    async fn begin(&self) -> Result<FlowSession> {
        let flow = recover_native_flow(&self.store, &self.id, self.claim().as_ref(), false).await?;
        anyhow::ensure!(
            !flow.finished && flow.failure.is_none(),
            "Flow is not ready to execute"
        );
        self.observe(&flow);
        Ok(flow)
    }
}

#[async_trait]
impl SkillExecutor for &CliFlowExecutor<'_> {
    async fn run_skill(
        &self,
        skill: &ConcreteSkill,
        ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        let flow = self.begin().await?;
        if skill.policy.human {
            let feedback = if flow.claim.is_some() {
                let task_id = flow.task_id.as_ref().context("claimed Flow has no Task")?;
                let task = self
                    .store
                    .get_task(task_id)
                    .await?
                    .context("Task disappeared")?;
                crate::controller::task::park_at_review(&self.store, &task, &flow).await?;
                None
            } else {
                crate::ops::flow_session::reserve(&self.store, &flow).await?
            };
            return Ok(match feedback {
                Some(feedback) => SkillOutcome::Completed {
                    feedback: Some(feedback),
                },
                None => SkillOutcome::Waiting,
            });
        }
        let mut flow = flow;
        if let Some(progress) = ctx.progress {
            print_skill_progress(progress, &skill.skill.name);
        } else {
            print_nested_skill_progress(&skill.skill.name);
        }
        loop {
            if !flow
                .current_attempt
                .as_ref()
                .is_some_and(crate::durable::FlowAttempt::completed)
            {
                execute_child(&self.store, &flow, self.launcher).await?;
                flow = self
                    .store
                    .flow(&self.id)
                    .await?
                    .context("Flow disappeared")?;
                self.observe(&flow);
            }
            match self.store.sqlite.flow_output(&self.id)? {
                Ok(outcome) => {
                    if let Some(attempt) = &flow.current_attempt {
                        *self.progress.lock().expect("Flow progress mutex poisoned") = self
                            .store
                            .sqlite
                            .input_final_answer(&attempt.run_id)?
                            .map(|answer| answer.text.trim().chars().take(2_000).collect());
                    }
                    return Ok(outcome);
                }
                Err(_) => {
                    flow = self.store.sqlite.correct_flow_output(
                        &self.id,
                        self.version(),
                        self.claim().as_ref(),
                    )?;
                    self.observe(&flow);
                }
            }
        }
    }

    async fn checkpoint(&self, cursor: &ExecutionCursor) -> Result<()> {
        let progress = self
            .progress
            .lock()
            .expect("Flow progress mutex poisoned")
            .take();
        let flow = self
            .store
            .checkpoint_flow(
                &self.id,
                self.version(),
                cursor,
                self.claim().as_ref(),
                progress.as_deref(),
            )
            .await?;
        self.observe(&flow);
        Ok(())
    }

    /// The child owns the effect; the driver consumes its Flow history.
    async fn run_op(&self, ops: &crate::engine::ConcreteOp, _ctx: ExecutionContext) -> Result<()> {
        let flow = self.begin().await?;
        if self.store.sqlite.flow_operation_completed(flow.id())? {
            return Ok(());
        }
        eprintln!("op: {}", ops.item.display_name());
        execute_child(&self.store, &flow, self.launcher).await
    }
}

/// Execute only the captured boundary named by the driver. No definition lookup
/// or driver lock: the parent owns traversal while this process owns the effect.
pub fn execute_step(id: &str, version: u64) -> Result<()> {
    let claim = std::env::var(crate::durable::TASK_WORKER_CLAIM_ENV)
        .ok()
        .map(|value| serde_json::from_str::<TaskWorkerClaim>(&value))
        .transpose()?;
    std::env::remove_var(crate::durable::TASK_WORKER_CLAIM_ENV);
    block_on_store(|store| async move {
        let flow = store
            .flow(id)
            .await?
            .context("Flow disappeared before step execution")?;
        anyhow::ensure!(
            flow.version == version && flow.claim == claim && !flow.finished,
            "Flow changed before step execution"
        );
        let exec = journal::current_exec_id().context("Flow step requires a registered Exec")?;
        let Some(ConcreteStep::Op(op)) = flow.current_step() else {
            anyhow::bail!("agent steps execute through lf skill");
        };
        let Some(start) =
            store
                .sqlite
                .begin_flow_operation(id, version, claim.as_ref(), Some(&exec))?
        else {
            return Ok(());
        };
        let cwd = flow.cwd.clone();
        let item = op.item.clone();
        let result = tokio::task::spawn_blocking(move || {
            crate::ops::execute_flow_ops(&cwd, &item, &NullProgress)
        })
        .await
        .context("Flow operation worker failed")?;
        store.sqlite.finish_flow_operation(
            id,
            version,
            claim.as_ref(),
            start,
            Some(&exec),
            result.is_ok(),
        )?;
        result.map_err(anyhow::Error::from)
    })
}

async fn execute_child(store: &SharedStore, flow: &FlowSession, cli: &Cli) -> Result<()> {
    let executable = std::env::current_exe().context("locate the executing Flow driver")?;
    let mut command = tokio::process::Command::new(executable);
    command
        .current_dir(&flow.cwd)
        .env_remove(crate::durable::TASK_WORKER_CLAIM_ENV);
    if matches!(flow.current_step(), Some(ConcreteStep::Op(_))) {
        command.args(["__flow-step", flow.id(), &flow.version.to_string()]);
    } else {
        command.args(cli.step_args());
        command.args([
            "--__flow-step",
            &flow_run::ActiveStep::of(flow).env_value()?,
            "skill",
            &flow.current().step,
        ]);
    }
    if let Some(claim) = &flow.claim {
        command.env(
            crate::durable::TASK_WORKER_CLAIM_ENV,
            serde_json::to_string(claim)?,
        );
    }
    let status = command
        .status()
        .await
        .context("could not execute captured Flow step")?;
    if status.code() == Some(130) {
        return Err(StepEnd::Interrupted.into());
    }
    if !status.success() {
        let selected = store.flow(flow.id()).await?.context("Flow disappeared")?;
        if let Some(attempt) = selected.current_attempt {
            let events = store.sqlite.input_events(&attempt.run_id)?;
            for envelope in events.iter().rev() {
                let event = &envelope["event"];
                if event["type"] == "turn_completed" && event["status"] == "interrupted" {
                    return Err(StepEnd::Interrupted.into());
                }
                if event["type"] == "error" {
                    anyhow::bail!(
                        "{}: {}: {}",
                        flow.current().step,
                        event["code"].as_str().unwrap_or("provider error"),
                        event["message"].as_str().unwrap_or("provider turn failed")
                    );
                }
            }
        }
        anyhow::bail!("{} process exited with {status}", flow.current().step);
    }
    Ok(())
}

fn print_skill_progress(progress: StepProgress, skill_name: &str) {
    let colors = Colors::new();
    eprintln!(
        "{dim}[{current}/{total}]{reset} {bold}{name}{reset}",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        current = progress.index + 1,
        total = progress.total,
        name = skill_name,
    );
}

fn print_nested_skill_progress(skill_name: &str) {
    let colors = Colors::new();
    eprintln!(
        "{dim}[*]{reset} {bold}{name}{reset}",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = skill_name,
    );
}

#[cfg(test)]
mod tests {
    use super::render_pipeline_lines;
    use crate::engine::{ConcreteStep, Flow};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn render_pipeline_lines_expands_xor_paths_on_separate_lines() {
        let temp = tempdir().unwrap();
        let skills = temp.path().join(".lf/skills/tend");
        fs::create_dir_all(&skills).unwrap();
        for name in ["scan-waves", "assess", "play-chord", "review-chord"] {
            fs::write(skills.join(format!("{name}.md")), format!("Fixture {name}")).unwrap();
        }
        let flows_dir = temp.path().join(".lf/flows/tend");
        fs::create_dir_all(&flows_dir).unwrap();
        fs::write(
            flows_dir.join("tune.yaml"),
            "- tend/play-chord\n- tend/review-chord\n",
        )
        .unwrap();

        let flow = Flow {
            name: "tend".to_string(),
            items: vec![
                crate::engine::flow::Step::Skill(crate::engine::flow::SkillStep {
                    skill: crate::engine::flow::Skill::named("tend/scan-waves"),
                    policy: crate::engine::OccurrencePolicy::default(),
                }),
                crate::engine::flow::Step::Xor(crate::engine::flow::XorDef {
                    router: Some("tend/assess".to_string()),
                    paths: [
                        (
                            "tune".to_string(),
                            crate::engine::flow::XorPath {
                                flow: Some("tend/tune".to_string()),
                                skill: None,
                                steps: Vec::new(),
                                description: "Adjust the chord".to_string(),
                            },
                        ),
                        (
                            "silence".to_string(),
                            crate::engine::flow::XorPath {
                                flow: None,
                                skill: None,
                                steps: Vec::new(),
                                description: "No-op".to_string(),
                            },
                        ),
                    ]
                    .into_iter()
                    .collect(),
                }),
            ],
        };

        let items = crate::engine::expand_flow(&flow, temp.path()).unwrap();
        let lines = render_pipeline_lines(&items);

        assert_eq!(
            lines,
            vec![
                "tend/scan-waves".to_string(),
                "[xor via tend/assess]".to_string(),
                "├─ silence".to_string(),
                "└─ tune → tend/play-chord → tend/review-chord".to_string(),
            ]
        );

        assert!(matches!(items[1], ConcreteStep::Xor(_)));
    }

    #[test]
    fn rendered_pipeline_lists_human_node_identity() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let flow = crate::engine::load_flow("task-design", &repo).unwrap();
        let items = crate::engine::expand_flow(&flow, &repo).unwrap();

        let lines = render_pipeline_lines(&items);
        assert_eq!(
            lines,
            vec![
                "kickoff".to_string(),
                "review-design [review:review_kickoff]".to_string(),
            ]
        );
    }

    #[test]
    fn validation_lists_human_nodes_hidden_in_xor_paths() {
        let repo = tempdir().unwrap();
        let flows = repo.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(
            flows.join("choice.yaml"),
            "- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - step:\n              id: revise_choice\n              name: implement\n          - step:\n              id: review_choice\n              name: review-design\n              human: true\n",
        )
        .unwrap();
        let flow = crate::engine::load_flow("choice", repo.path()).unwrap();
        let human = crate::engine::human_occurrence_ids(&flow, repo.path()).unwrap();
        assert_eq!(human, vec!["review_choice"]);
        let items = crate::engine::expand_flow(&flow, repo.path()).unwrap();
        assert!(render_pipeline_lines(&items)
            .iter()
            .any(|line| line.contains("review-design [review:review_choice]")));
    }
}
