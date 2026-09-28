use crate::durable::{FlowInvocation, TaskFlowBlocker, TaskWorkerClaim, WorkRef};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::transitions::{FlowDecision, FlowVerdict};
use crate::engine::{
    expand_flow, human_occurrence_ids, ConcreteSkill, ConcreteStep, ConcreteXor, ExecutionContext,
    ExecutionCursor, Flow, FlowEngine, FlowOutcome, SkillExecutor, SkillOutcome, StepProgress,
};
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::Cli;
use crate::ops::{commit_workflow, flow_run, CommitOptions, NullProgress, WorkBinding};
use crate::run_record::{RunFlowMembership, RunFlowStep};
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
    print_pipeline_header(&flow.name, &items, repo)?;
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
    for line in render_pipeline_lines(&items, repo)? {
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
    let flow = FlowInvocation {
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
        "decide" => {
            let decision = match args.first().map(String::as_str) {
                Some("advance") => FlowDecision::Advance,
                Some("iterate") => FlowDecision::Iterate,
                _ => anyhow::bail!("usage: lf flow decide advance|iterate SUMMARY"),
            };
            let summary = args[1..].join(" ");
            anyhow::ensure!(
                !summary.trim().is_empty(),
                "decision requires evidence or direction"
            );
            let verdict = FlowVerdict { decision, summary };
            let step = active_step()?;
            let run = active_run_id()?;
            block_on_store(|store| async move {
                store
                    .record_flow_decision(&step.invocation, step.version, &run, &verdict)
                    .await
                    .map_err(Into::into)
            })?;
            println!("Decision recorded; it takes effect when this Run finishes successfully.");
            Ok(())
        }
        "route" => {
            anyhow::ensure!(args.len() == 1, "usage: lf flow route PATH");
            let run = active_run_id()?;
            let path = args[0].clone();
            let step = active_step()?;
            block_on_store(|store| async move {
                store
                    .record_flow_path(&step.invocation, step.version, &run, &path)
                    .await
                    .map_err(Into::into)
            })?;
            println!("Route recorded; it takes effect when this Run finishes successfully.");
            Ok(())
        }
        "blocked" => {
            let reason = args.join(" ");
            anyhow::ensure!(!reason.trim().is_empty(), "usage: lf flow blocked REASON");
            let run_id = active_run_id()?;
            let step = active_step()?;
            block_on_store(|store| async move {
                let key = store
                    .flow_blocker_key(&step.invocation, step.version, &run_id)
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
                    crate::ops::task::task_resume(&task.plan.identifier, None, cli.model.clone())?;
                    return Ok(());
                }
            }
            let flow = if args.len() == 2 {
                let _driver = flow_run::driver_lock(id)?;
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

fn active_run_id() -> Result<crate::durable::RunId> {
    crate::durable::RunId::parse(
        &std::env::var(crate::durable::RUN_ID_ENV)
            .context("this operation requires the active decision Run")?,
    )
    .map_err(Into::into)
}

fn active_step() -> Result<flow_run::ActiveStep> {
    flow_run::token()?.ok_or_else(|| anyhow!("this operation runs inside a Flow step's Run"))
}

/// Drive a saved Flow from its row with the `lf` launcher.
fn drive_saved(
    runtime: &tokio::runtime::Runtime,
    store: SharedStore,
    flow: FlowInvocation,
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

/// Drive one invocation from its row until it completes, waits or blocks.
/// `claim` is the Task worker's, held until the driver stops; every write is
/// fenced by it. The engine owns traversal; the launcher owns how a step's
/// provider runs and how a review parks.
pub(crate) async fn drive(
    store: SharedStore,
    flow: FlowInvocation,
    claim: Option<TaskWorkerClaim>,
    launcher: &dyn StepLauncher,
) -> Result<FlowOutcome> {
    let id = flow.id().to_owned();
    let _driver = flow_run::driver_lock(&id)?;
    let mut flow = store.recover_flow(&id, claim.as_ref()).await?;
    if flow.finished {
        println!("Flow {} is already finished.", flow.invocation.flow);
        return Ok(FlowOutcome::Completed);
    }
    if let Some(failure) = &flow.failure {
        anyhow::bail!(
            "Flow {id} is blocked: {}; resume with `lf flow resume {id} --retry`",
            failure.reason
        );
    }
    let _flow_env = EnvVarGuard::set("LOOPFLOW_FLOW_NAME", &flow.invocation.flow);
    let executor = CliFlowExecutor {
        store: store.clone(),
        id: id.clone(),
        version: Mutex::new(flow.version),
        claim: Mutex::new(claim),
        progress: Mutex::new(None),
        launcher,
    };
    let steps = flow.invocation.steps.clone();
    let outcome = FlowEngine::new(&executor)
        .run_with_cursor(&steps, &mut flow.cursor)
        .await;
    let version = executor.version();
    let claim = executor.claim();
    let progress = executor
        .progress
        .lock()
        .expect("Flow progress mutex poisoned")
        .take();
    match outcome {
        Ok(FlowOutcome::Completed) => {
            store
                .end_flow(&id, claim.as_ref(), progress.as_deref().unwrap_or_default())
                .await?;
            Ok(FlowOutcome::Completed)
        }
        Ok(FlowOutcome::Waiting) => Ok(FlowOutcome::Waiting),
        Ok(FlowOutcome::Blocked(reason)) => {
            store
                .fail_flow(&id, version, claim.as_ref(), &TaskFlowBlocker::now(&reason))
                .await?;
            anyhow::bail!("Flow {id} blocked: {reason}; resume with `lf flow resume {id} --retry`")
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
    }
}

fn record<T>(written: crate::store::StoreResult<T>) {
    if let Err(error) = written {
        tracing::warn!(%error, "Flow failure was not recorded on its row");
    }
}

fn print_pipeline_header(flow_name: &str, items: &[ConcreteStep], repo: &Path) -> Result<()> {
    let colors = Colors::new();
    let lines = render_pipeline_lines(items, repo)?;
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
    Ok(())
}

fn render_pipeline_lines(items: &[ConcreteStep], repo: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for item in items {
        lines.extend(render_pipeline_item(item, repo)?);
    }
    Ok(lines)
}

fn render_pipeline_item(item: &ConcreteStep, repo: &Path) -> Result<Vec<String>> {
    match item {
        ConcreteStep::Skill(skill) if skill.policy.human => Ok(vec![format!(
            "{} [review:{}]",
            skill.skill.name,
            skill
                .policy
                .id
                .as_deref()
                .expect("validated review node has an id"),
        )]),
        ConcreteStep::Skill(skill) => Ok(vec![skill.skill.name.clone()]),
        ConcreteStep::Op(ops) => Ok(vec![format!("op: {}", ops.item.display_name())]),
        ConcreteStep::Xor(branch) => render_branch_item("xor", branch, repo),
    }
}

fn render_branch_item(kind: &str, branch: &ConcreteXor, repo: &Path) -> Result<Vec<String>> {
    render_branch_pipeline(kind, &branch.router.name, &branch.paths, repo)
}

fn render_branch_pipeline(
    kind: &str,
    router: &str,
    paths: &std::collections::HashMap<String, crate::engine::ConcretePath>,
    repo: &Path,
) -> Result<Vec<String>> {
    let mut lines = vec![format!("[{kind} via {router}]")];
    let mut keys: Vec<&String> = paths.keys().collect();
    keys.sort();

    for (index, key) in keys.into_iter().enumerate() {
        let path = paths
            .get(key)
            .expect("branch path key collected from map should exist");
        let nested = render_pipeline_lines(&path.steps, repo)?;
        let branch_prefix = tree_prefix(index, paths.len());
        if nested.is_empty() {
            lines.push(format!("{branch_prefix} {key}"));
            continue;
        }

        let nested_chain = nested.join(" → ");
        lines.push(format!("{branch_prefix} {key} → {nested_chain}"));
    }

    Ok(lines)
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
    async fn review(&self, flow: &FlowInvocation, skill: &ConcreteSkill) -> Result<Option<String>>;

    /// Run the step's provider to completion. `flow.current_attempt` is the
    /// reserved Run the launch publishes. Returns the step's progress summary.
    async fn launch(
        &self,
        flow: &FlowInvocation,
        skill: &ConcreteSkill,
        ctx: &ExecutionContext,
        claim: Option<&TaskWorkerClaim>,
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

    fn observe(&self, flow: &FlowInvocation) {
        *self.version.lock().expect("Flow version mutex poisoned") = flow.version;
        if flow.claim.is_none() {
            *self.claim.lock().expect("Flow claim mutex poisoned") = None;
        }
    }

    /// The row at the step about to run, with any earlier attempt settled.
    async fn begin(&self) -> Result<FlowInvocation> {
        let flow = self
            .store
            .recover_flow(&self.id, self.claim().as_ref())
            .await?;
        anyhow::ensure!(
            !flow.finished && flow.failure.is_none(),
            "Flow is not ready to execute"
        );
        self.observe(&flow);
        Ok(flow)
    }

    /// The step's Run, stored before anything launches it.
    async fn reserve(&self, flow: FlowInvocation) -> Result<FlowInvocation> {
        let flow = self
            .store
            .reserve_attempt(&self.id, flow.version, self.claim().as_ref())
            .await?;
        self.observe(&flow);
        Ok(flow)
    }

    /// The outcome the step's Run recorded on the row.
    async fn finish(&self) -> Result<SkillOutcome> {
        let flow = self
            .store
            .flow(&self.id)
            .await?
            .ok_or_else(|| anyhow!("Flow {} disappeared", self.id))?;
        let leaf = flow.cursor.leaf();
        Ok(if let Some(route) = &leaf.route {
            SkillOutcome::Routed(route.clone())
        } else {
            leaf.progress.verdict.clone().map_or(
                SkillOutcome::Completed { feedback: None },
                SkillOutcome::Decided,
            )
        })
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
        let flow = self.reserve(flow).await?;
        if let Some(progress) = ctx.progress {
            print_skill_progress(progress, &skill.skill.name);
        } else {
            print_nested_skill_progress(&skill.skill.name);
        }
        // A completed attempt at this position is the step's completion.
        if flow
            .current_attempt
            .as_ref()
            .is_some_and(crate::durable::FlowAttempt::completed)
        {
            return self.finish().await;
        }
        let progress = self
            .launcher
            .launch(&flow, skill, &ctx, self.claim().as_ref())
            .await?;
        *self.progress.lock().expect("Flow progress mutex poisoned") = progress;
        self.finish().await
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

    /// An operation is an attempt like a skill launch: its Run row is the
    /// receipt recovery reads, so an interrupted operation blocks for
    /// inspection instead of replaying its side effect.
    async fn run_op(&self, ops: &crate::engine::ConcreteOp, _ctx: ExecutionContext) -> Result<()> {
        let flow = self.reserve(self.begin().await?).await?;
        let name = ops.item.display_name();
        eprintln!("op: {name}");
        let attempt = flow
            .current_attempt
            .as_ref()
            .ok_or_else(|| anyhow!("op: {name} has no reserved Run"))?;
        if attempt.completed() {
            return Ok(());
        }
        let claim = self.claim();
        let store = self.store.clone();
        let (id, version) = (self.id.clone(), flow.version);
        let capture = crate::run_record::CaptureHandle::begin_reserved_with_context(
            crate::run_record::RunSpec {
                harness: "loopflow".into(),
                model: None,
                surface: "operation".into(),
                cwd: flow.cwd.clone(),
                repo: Some(flow.cwd.clone()),
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                flow: RunFlowMembership::Step(RunFlowStep::of(&flow)?),
                work: flow.declared_work(),
            },
            attempt.run_id.clone(),
            &crate::trace::PreparedTurnContext::from_prompts(
                "Loopflow mechanical Flow boundary",
                &name,
            ),
            move |run_id| {
                store
                    .sqlite
                    .publish_attempt(&id, version, run_id, claim.as_ref(), "loopflow", None)
            },
        )?;
        capture.record_input("operation", &name);
        let cwd = flow.cwd.clone();
        let item = ops.item.clone();
        let result = tokio::task::spawn_blocking(move || {
            crate::ops::execute_flow_ops(&cwd, &item, &NullProgress)
        })
        .await
        .map_err(|error| anyhow!("op: {name} worker failed: {error}"))?;
        capture.finish(if result.is_ok() {
            "completed"
        } else {
            "failed"
        })?;
        result.map_err(|error| anyhow!("op: {name} failed: {error}"))
    }
}

/// The saved Flow's launcher: each step is an `lf <skill>` in the Flow's cwd.
struct SavedLauncher {
    cli: Cli,
    store: SharedStore,
}

#[async_trait]
impl StepLauncher for SavedLauncher {
    async fn review(&self, flow: &FlowInvocation, skill: &ConcreteSkill) -> Result<Option<String>> {
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
        flow: &FlowInvocation,
        skill: &ConcreteSkill,
        ctx: &ExecutionContext,
        _claim: Option<&TaskWorkerClaim>,
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
        if skill.policy.repeat.is_some() {
            message.push_str("\n\nRecord this occurrence's decision with `lf flow decide advance \"evidence\"` or `lf flow decide iterate \"next action and proof\"`. If progress is stalled, use `lf flow blocked \"reason, attempted direction, evidence, and question\"`; it opens one Ask running unblock and returns the human's summary. Reassess afterward. Invalid commands return correction feedback; correct the decision here without replaying implementation. A recorded decision is accepted only after this Run succeeds.");
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
        let lines = render_pipeline_lines(&items, temp.path()).unwrap();

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

        let lines = render_pipeline_lines(&items, &repo).unwrap();
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
        assert!(render_pipeline_lines(&items, repo.path())
            .unwrap()
            .iter()
            .any(|line| line.contains("review-design [review:review_choice]")));
    }
}
