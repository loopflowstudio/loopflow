use crate::durable::{FlowSession, TaskFlowBlocker, TaskWorkerClaim, WorkRef};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::{
    expand_flow, human_occurrence_ids, ConcreteSkill, ConcreteStep, ConcreteXor, ExecutionContext,
    ExecutionCursor, Flow, FlowEngine, FlowOutcome, SkillExecutor, SkillOutcome, StepProgress,
};
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::Cli;
use crate::ops::{commit_workflow, flow_run, CommitOptions, NullProgress, WorkBinding};
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
    let fields = |extra: LfEventFields| LfEventFields {
        flow: Some(flow_name.to_string()),
        ..extra
    };
    journal::emit(
        repo,
        LfNode::Flow,
        LfEventType::Started,
        fields(LfEventFields::default()),
    );
    let _flow_env = EnvVarGuard::set("LOOPFLOW_FLOW_NAME", flow_name);
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
    let result = drive_saved(&runtime, store, flow, cli);
    match &result {
        Ok(outcome) => journal::emit(
            repo,
            LfNode::Flow,
            if *outcome == FlowOutcome::Completed {
                LfEventType::Completed
            } else {
                LfEventType::Escalated
            },
            fields(LfEventFields::default()),
        ),
        Err(err) => journal::emit(
            repo,
            LfNode::Flow,
            LfEventType::Errored,
            fields(LfEventFields {
                error: Some(err.to_string()),
                ..LfEventFields::default()
            }),
        ),
    }
    report_outcome(result?)
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
                report_outcome(drive_saved(&runtime, store, flow, cli)?)
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

/// Drive a saved Flow from its row with the `lf` launcher.
fn drive_saved(
    runtime: &tokio::runtime::Runtime,
    store: SharedStore,
    flow: FlowSession,
    cli: &Cli,
) -> Result<FlowOutcome> {
    let mut launch = cli.launch_options();
    launch.as_work = None;
    launch.task = flow.task_id.as_ref().map(ToString::to_string);
    launch.wave = flow.wave_id.as_ref().map(ToString::to_string);
    if launch.model.is_none() {
        launch.model = flow.model.clone();
    }
    launch.bound_cwd = Some(flow.cwd.clone());
    let launcher = SavedLauncher {
        cli: launch,
        store: store.clone(),
    };
    runtime.block_on(drive(store, flow, None, &launcher))
}

/// A step ended without a result. The driver releases the position so the
/// step runs again as a new attempt; `Released` still fails the driver with
/// its reason, `Interrupted` is a stop the operator asked for.
#[derive(Debug)]
pub(crate) enum StepEnd {
    Released(String),
    Interrupted,
}

impl std::fmt::Display for StepEnd {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Released(reason) => formatter.write_str(reason),
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
        let Some(exec) = store.sqlite.pending_flow_operation_exec(flow.id())? else {
            return Ok(());
        };
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
/// fenced by it. The engine owns traversal; the launcher owns how a step's
/// provider runs and how a review parks.
pub(crate) async fn drive(
    store: SharedStore,
    flow: FlowSession,
    claim: Option<TaskWorkerClaim>,
    launcher: &dyn StepLauncher,
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
                Some(StepEnd::Released(reason)) => {
                    record(store.release_flow(&id, version, claim.as_ref()).await);
                    anyhow::bail!("{reason}")
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

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
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

/// What a driver adds around the one executor: how a step's provider runs and
/// how a review parks. The saved Flow launches `lf`; a Task's worker adds its
/// harness, steers and attachment.
#[async_trait]
pub(crate) trait StepLauncher: Send + Sync {
    /// Park at the review: its feedback once the review completed, `None`
    /// while it waits.
    async fn review(&self, flow: &FlowSession, skill: &ConcreteSkill) -> Result<Option<String>>;

    /// Run the step's provider to completion. `flow.current_attempt` is the
    /// reserved Run the launch publishes; `flow.claim` is the validated worker
    /// claim. Returns the step's progress summary.
    async fn launch(
        &self,
        flow: &FlowSession,
        skill: &ConcreteSkill,
        ctx: &ExecutionContext,
    ) -> Result<Option<String>>;
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
    launcher: &'a dyn StepLauncher,
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

    /// The step's Run, stored before anything launches it.
    async fn reserve(&self, flow: FlowSession) -> Result<FlowSession> {
        let flow = self
            .store
            .reserve_attempt(&self.id, flow.version, self.claim().as_ref())
            .await?;
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
            return Ok(match self.launcher.review(&flow, skill).await? {
                Some(feedback) => SkillOutcome::Completed {
                    feedback: Some(feedback),
                },
                None => SkillOutcome::Waiting,
            });
        }
        let mut flow = self.reserve(flow).await?;
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
                let mut context = ctx.clone();
                context.direction = flow
                    .cursor
                    .leaf()
                    .progress
                    .direction
                    .clone()
                    .or(context.direction);
                let progress = self.launcher.launch(&flow, skill, &context).await?;
                *self.progress.lock().expect("Flow progress mutex poisoned") = progress;
            }
            match self.store.sqlite.flow_output(&self.id)? {
                Ok(outcome) => return Ok(outcome),
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
        execute_child(&flow).await
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
        let Some(ConcreteStep::Op(op)) = flow.current_step() else {
            anyhow::bail!("captured boundary is not a mechanical operation");
        };
        let exec = journal::current_exec_id().context("Flow step requires a registered Exec")?;
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

async fn execute_child(flow: &FlowSession) -> Result<()> {
    let executable = std::env::current_exe().context("locate the executing Flow driver")?;
    let mut command = tokio::process::Command::new(executable);
    command
        .args(["__flow-step", flow.id(), &flow.version.to_string()])
        .current_dir(&flow.cwd)
        .env_remove(crate::durable::TASK_WORKER_CLAIM_ENV);
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
    anyhow::ensure!(status.success(), "Flow step process exited with {status}");
    Ok(())
}

/// The saved Flow's launcher: each step is an `lf <skill>` in the Flow's cwd.
struct SavedLauncher {
    cli: Cli,
    store: SharedStore,
}

#[async_trait]
impl StepLauncher for SavedLauncher {
    async fn review(&self, flow: &FlowSession, skill: &ConcreteSkill) -> Result<Option<String>> {
        let feedback = crate::ops::flow_session::reserve(&self.store, flow).await?;
        if feedback.is_none() {
            eprintln!(
                "Flow {} is waiting at {}. Open its Flow Session with lf session.",
                flow.id(),
                skill.skill.name
            );
        }
        Ok(feedback)
    }

    async fn launch(
        &self,
        flow: &FlowSession,
        skill: &ConcreteSkill,
        ctx: &ExecutionContext,
    ) -> Result<Option<String>> {
        let _token = EnvVarGuard::set(
            flow_run::FLOW_STEP_ENV,
            &flow_run::ActiveStep::of(flow).env_value()?,
        );
        let mut message = flow.message.clone().unwrap_or_default();
        if let Some(direction) = &ctx.direction {
            message.push_str(&format!(
                "\n\nPrevious step feedback or iteration direction:\n{direction}"
            ));
        }
        if let Some(output) = flow
            .current_step()
            .and_then(crate::engine::flow_output::FlowOutput::for_step_instructions)
        {
            message.push_str(&output);
        }
        let mut launch = self.cli.launch_options();
        launch.batch = true;
        launch.interactive = false;
        launch.tui = false;
        launch.ide = false;
        let work = flow.declared_work();
        let repo = flow.cwd.clone();
        let result = run_skill_with_journal(
            &repo,
            &skill.skill.name,
            ctx.progress.map(|p| p.index),
            || {
                crate::lf::commands::run::run_saved(
                    &skill.skill,
                    Some(&message),
                    &launch,
                    &repo,
                    work,
                )?;
                if self.cli.task.is_none() && self.cli.wave.is_none() {
                    commit_skill_work(&repo, &skill.skill.name)?;
                }
                Ok(())
            },
        );
        if let Err(error) = result {
            anyhow::bail!("{} Run failed: {error:#}", skill.skill.name);
        }
        Ok(None)
    }
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

fn run_skill_with_journal(
    repo: &Path,
    skill_name: &str,
    index: Option<usize>,
    run: impl FnOnce() -> Result<()>,
) -> Result<()> {
    journal::emit(
        repo,
        LfNode::Skill,
        LfEventType::Started,
        LfEventFields {
            skill: Some(skill_name.to_string()),
            index: index.map(|value| value as u32),
            ..LfEventFields::default()
        },
    );
    let result = run();
    match &result {
        Ok(_) => journal::emit(
            repo,
            LfNode::Skill,
            LfEventType::Completed,
            LfEventFields {
                skill: Some(skill_name.to_string()),
                index: index.map(|value| value as u32),
                ..LfEventFields::default()
            },
        ),
        Err(err) => journal::emit(
            repo,
            LfNode::Skill,
            LfEventType::Errored,
            LfEventFields {
                skill: Some(skill_name.to_string()),
                index: index.map(|value| value as u32),
                error: Some(err.to_string()),
                ..LfEventFields::default()
            },
        ),
    }
    result
}

/// Commit any uncommitted changes left by the previous skill.
pub(crate) fn commit_skill_work(repo: &Path, skill_name: &str) -> Result<bool> {
    let options = CommitOptions {
        add: true,
        message: Some(format!("lf commit: {skill_name}")),
        ..CommitOptions::for_task(skill_name)
    };
    commit_workflow(repo, &options, &NullProgress).map_err(Into::into)
}
#[cfg(test)]
mod tests {
    use super::render_pipeline_lines;
    use crate::engine::{ConcreteStep, Flow};
    use std::fs;
    use tempfile::tempdir;

    struct LoopProof {
        store: crate::store::SharedStore,
        interrupted: std::sync::atomic::AtomicBool,
        visits: std::sync::Mutex<Vec<(String, String)>>,
    }

    #[async_trait::async_trait]
    impl super::StepLauncher for LoopProof {
        async fn review(
            &self,
            _: &crate::durable::FlowSession,
            _: &crate::engine::ConcreteSkill,
        ) -> anyhow::Result<Option<String>> {
            unreachable!("fixture has no human reviews")
        }

        async fn launch(
            &self,
            flow: &crate::durable::FlowSession,
            skill: &crate::engine::ConcreteSkill,
            _: &crate::engine::ExecutionContext,
        ) -> anyhow::Result<Option<String>> {
            assert!(flow.claim.is_none());
            let run = &flow.current_attempt.as_ref().unwrap().run_id;
            self.store.sqlite.publish_attempt(
                flow.id(),
                flow.version,
                self.store.sqlite.captured_sequence(run).unwrap().unwrap(),
                None,
                "proof",
                None,
            )?;
            let actor = self.store.sqlite.test_flow_turn(run);
            self.visits
                .lock()
                .unwrap()
                .push((skill.skill.name.clone(), flow.id().to_owned()));
            if flow.parent_id.is_some()
                && !self
                    .interrupted
                    .swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                self.store
                    .sqlite
                    .test_finish_flow_turn(&actor, "interrupted");
                return Err(super::StepEnd::Interrupted.into());
            }
            if skill.policy.repeat.is_some() {
                self.store.sqlite.test_decision_output(
                    &actor,
                    &crate::engine::transitions::FlowVerdict {
                        decision: if flow.cursor.iteration < 2 {
                            crate::engine::transitions::FlowDecision::Iterate
                        } else {
                            crate::engine::transitions::FlowDecision::Advance
                        },
                        summary: "repeat until the second pass".into(),
                    },
                )?;
            }
            self.store.sqlite.test_finish_flow_turn(&actor, "completed");
            Ok(None)
        }
    }

    #[test]
    fn taskless_driver_resumes_the_same_child_then_creates_a_sibling_pass() {
        let guard = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let store = std::sync::Arc::new(
                crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                    guard.home().join("loopflow.db"),
                ))
                .await
                .unwrap(),
            );
            let step = |id: &str, from: Option<&str>| {
                crate::engine::ConcreteStep::Skill(crate::engine::ConcreteSkill {
                    skill: crate::engine::Skill::named(id),
                    flow_parents: vec![],
                    policy: crate::engine::OccurrencePolicy {
                        id: Some(id.into()),
                        human: false,
                        repeat: from
                            .map(|from| crate::engine::flow::RepeatPolicy { from: from.into() }),
                    },
                })
            };
            let flow = store
                .create_flow(crate::durable::FlowSession {
                    parent_id: None,
                    invocation: crate::engine::invocation::QueuedInvocation::new(
                        "runtime-proof",
                        vec![
                            step("work", None),
                            step("decision", Some("work")),
                            step("after", None),
                        ],
                    )
                    .unwrap(),
                    cursor: Default::default(),
                    version: 0,
                    task_id: None,
                    wave_id: None,
                    cwd: guard.home().to_owned(),
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
                .await
                .unwrap();
            let launcher = LoopProof {
                store: store.clone(),
                interrupted: false.into(),
                visits: Default::default(),
            };
            assert_eq!(
                super::drive(store.clone(), flow.clone(), None, &launcher)
                    .await
                    .unwrap(),
                crate::engine::FlowOutcome::Waiting
            );
            let interrupted = store.sqlite.active_flow(flow.id()).unwrap();
            assert_eq!(interrupted.parent_id.as_deref(), Some(flow.id()));
            assert_eq!(interrupted.cursor.index, 0);
            assert_eq!(
                super::drive(store.clone(), flow.clone(), None, &launcher)
                    .await
                    .unwrap(),
                crate::engine::FlowOutcome::Completed
            );
            let visits = launcher.visits.lock().unwrap().clone();
            assert_eq!(
                visits
                    .iter()
                    .map(|(step, _)| step.as_str())
                    .collect::<Vec<_>>(),
                ["work", "decision", "work", "work", "decision", "work", "decision", "after"]
            );
            assert!(visits[2..5].iter().all(|(_, id)| id == interrupted.id()));
            assert_eq!(visits[5].1, visits[6].1);
            assert_ne!(visits[5].1, visits[2].1);
            assert_eq!(visits[7].1, flow.id());
            let sibling = store.flow(&visits[5].1).await.unwrap().unwrap();
            assert_eq!(sibling.parent_id.as_deref(), Some(flow.id()));
            assert!(sibling.finished);
            let conn = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
            assert_eq!(
                conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                0
            );
        });
    }

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
