use crate::durable::{FlowInvocation, WorkRef};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::transitions::{FlowDecision, FlowVerdict};
use crate::engine::{
    expand_flow, human_occurrence_ids, ConcreteSkill, ConcreteStep, ConcreteXor, ExecutionContext,
    ExecutionCursor, Flow, FlowEngine, FlowOutcome, SkillExecutor, SkillOutcome, StepProgress,
};
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::{Cli, FlowCommand};
use crate::ops::{commit_workflow, flow_run, CommitOptions, NullProgress, WorkBinding};
use crate::run_record::{RunFlowMembership, RunFlowStep};
use crate::store::SharedStore;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

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
    let flow = crate::engine::flow::load_authored_flow(name, repo)?;
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
    let flow = crate::engine::flow::load_authored_flow(name, repo)?;
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
        failure: None,
        finished: false,
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

pub fn control(command: &FlowCommand, cli: &Cli, repo: &Path) -> Result<()> {
    match command {
        FlowCommand::Decide { decision, summary } => {
            let decision = match decision.as_str() {
                "advance" => FlowDecision::Advance,
                "iterate" => FlowDecision::Iterate,
                _ => anyhow::bail!("unknown decision {decision}"),
            };
            let summary = summary.join(" ");
            anyhow::ensure!(
                !summary.trim().is_empty(),
                "decision requires evidence or direction"
            );
            let verdict = FlowVerdict { decision, summary };
            if let Some(step) = flow_run::token()? {
                let run = active_run_id()?;
                block_on_store(|store| async move {
                    store
                        .record_flow_decision(&step.invocation, step.version, &run, &verdict)
                        .await
                        .map_err(Into::into)
                })?;
            } else {
                crate::ops::task::task_verdict(repo, verdict.decision, &verdict.summary)?;
            }
            println!("Decision recorded; it takes effect when this Run finishes successfully.");
            Ok(())
        }
        FlowCommand::Route { path } => {
            let run = active_run_id()?;
            let path = path.clone();
            let step = flow_run::token()?;
            block_on_store(|store| async move {
                match step {
                    Some(step) => {
                        store
                            .record_flow_path(&step.invocation, step.version, &run, &path)
                            .await?
                    }
                    None => {
                        let task = crate::ops::task::task_for_checkout(&store, repo)
                            .await?
                            .ok_or_else(|| anyhow!("no Task in this checkout"))?;
                        store.record_flow_route(&task.id, &run, &path).await?;
                    }
                }
                Ok(())
            })?;
            println!("Route recorded; it takes effect when this Run finishes successfully.");
            Ok(())
        }
        FlowCommand::Blocked { reason } => {
            let reason = reason.join(" ");
            anyhow::ensure!(!reason.trim().is_empty(), "usage: lf flow blocked REASON");
            let run_id = active_run_id()?;
            let step = flow_run::token()?;
            block_on_store(|store| async move {
                let key = match step {
                    Some(step) => {
                        store
                            .flow_blocker_key(&step.invocation, step.version, &run_id)
                            .await?
                    }
                    None => {
                        let task = crate::ops::task::task_for_checkout(&store, repo)
                            .await?
                            .ok_or_else(|| anyhow!("no current Flow decision"))?;
                        let position = store
                            .flow_position(&task.id)
                            .await?
                            .ok_or_else(|| anyhow!("Task has no current Flow"))?;
                        anyhow::ensure!(
                            position
                                .claim
                                .as_ref()
                                .and_then(|c| c.worker_run_id.as_ref())
                                == Some(&run_id)
                                && !position.is_human(),
                            "this Run does not own the Task decision"
                        );
                        anyhow::ensure!(
                            matches!(position.current_plan(), ConcreteStep::Skill(s) if s.repeat.is_some()),
                            "this step does not own a loop decision"
                        );
                        format!(
                            "task:{}:{}:{}",
                            task.id,
                            position.invocation.id,
                            position.cursor.boundary_key()
                        )
                    }
                };
                let summary =
                    crate::ops::human_session::ask_once(&store, &key, &reason, Some("unblock"))
                        .await?;
                println!("Session complete: {summary}\nReassess the current evidence before choosing Advance or Iterate. This returned feedback, not a navigation decision.");
                Ok(())
            })
        }
        FlowCommand::Resume { invocation, retry } => {
            let id = invocation.as_str();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let store = runtime.block_on(open_flow_store())?;
            let flow = runtime
                .block_on(store.flow(id))?
                .ok_or_else(|| anyhow!("Flow {id} has no invocation row"))?;
            let flow = if *retry {
                let _driver = flow_run::driver_lock(id)?;
                runtime.block_on(store.retry_flow(id))?
            } else {
                flow
            };
            let argv = std::env::args().collect::<Vec<_>>();
            let cwd = flow.cwd.clone();
            journal::with_runtime(&cwd, &argv, || {
                report_outcome(drive_saved(&runtime, store, flow, cli)?)
            })
        }
        _ => anyhow::bail!("not a Flow control: {command:?}"),
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

/// Drive one invocation from its row until it completes, waits or blocks.
fn drive_saved(
    runtime: &tokio::runtime::Runtime,
    store: SharedStore,
    flow: FlowInvocation,
    cli: &Cli,
) -> Result<FlowOutcome> {
    let id = flow.id().to_owned();
    let _driver = flow_run::driver_lock(&id)?;
    let mut flow = runtime.block_on(store.recover_flow(&id))?;
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
    let mut launch = cli.launch_options();
    launch.as_work = None;
    launch.task = flow.task_id.as_ref().map(ToString::to_string);
    launch.wave = flow.wave_id.as_ref().map(ToString::to_string);
    if launch.model.is_none() {
        launch.model = flow.model.clone();
    }
    launch.bound_cwd = Some(flow.cwd.clone());
    let executor = CliFlowExecutor {
        cli: &launch,
        store: store.clone(),
        id: id.clone(),
        message: flow.message.clone(),
        repo: flow.cwd.clone(),
        version: std::sync::Mutex::new(flow.version),
    };
    let steps = flow.invocation.steps.clone();
    let outcome =
        runtime.block_on(FlowEngine::new(&executor).run_with_cursor(&steps, &mut flow.cursor));
    let version = *executor
        .version
        .lock()
        .expect("Flow version mutex poisoned");
    match outcome {
        Ok(FlowOutcome::Completed) => {
            runtime.block_on(store.end_flow(&id))?;
            Ok(FlowOutcome::Completed)
        }
        Ok(FlowOutcome::Waiting) => Ok(FlowOutcome::Waiting),
        Ok(FlowOutcome::Blocked(reason)) => {
            runtime.block_on(store.fail_flow(&id, version, &reason))?;
            anyhow::bail!("Flow {id} blocked: {reason}; resume with `lf flow resume {id} --retry`")
        }
        Err(error) => {
            runtime.block_on(store.fail_flow(&id, version, &format!("{error:#}")))?;
            Err(error.context(format!(
                "Flow {id} blocked; resume with `lf flow resume {id} --retry`"
            )))
        }
    }
}

fn print_pipeline_header(flow_name: &str, items: &[ConcreteStep]) {
    let colors = Colors::new();
    let pipeline = render_pipeline_lines(items)
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
    let mut lines = Vec::new();
    for item in items {
        match item {
            ConcreteStep::Skill(skill) if skill.human => lines.push(format!(
                "{} [review:{}]",
                skill.skill.name,
                skill
                    .id
                    .as_deref()
                    .expect("validated review node has an id"),
            )),
            ConcreteStep::Skill(skill) => lines.push(skill.skill.name.clone()),
            ConcreteStep::Command(command) => lines.push(command.item.to_string()),
            ConcreteStep::Xor(branch) => lines.extend(render_branch_lines(branch)),
        }
    }
    lines
}

fn render_branch_lines(branch: &ConcreteXor) -> Vec<String> {
    let mut lines = vec![format!("[xor via {}]", branch.router.name)];
    let paths = &branch.paths;
    let mut keys: Vec<&String> = paths.keys().collect();
    keys.sort();

    for (index, key) in keys.into_iter().enumerate() {
        let path = paths
            .get(key)
            .expect("branch path key collected from map should exist");
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

/// The one Flow executor: every step reads and writes the invocation row.
struct CliFlowExecutor<'a> {
    cli: &'a Cli,
    store: SharedStore,
    id: String,
    message: Option<String>,
    repo: PathBuf,
    version: std::sync::Mutex<u64>,
}

impl CliFlowExecutor<'_> {
    fn version(&self) -> u64 {
        *self.version.lock().expect("Flow version mutex poisoned")
    }

    fn observe(&self, flow: &FlowInvocation) {
        *self.version.lock().expect("Flow version mutex poisoned") = flow.version;
    }

    /// The row at the step about to run, with any earlier attempt settled.
    async fn begin(&self) -> Result<FlowInvocation> {
        let flow = self.store.recover_flow(&self.id).await?;
        anyhow::ensure!(
            !flow.finished && flow.failure.is_none(),
            "Flow is not ready to execute"
        );
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
        if skill.human {
            if let Some(feedback) = crate::ops::flow_session::reserve(&self.store, &flow).await? {
                return Ok(SkillOutcome::Completed {
                    feedback: Some(feedback),
                });
            }
            eprintln!(
                "Flow {} is waiting at {}. Open its Flow Session with lf session.",
                self.id, skill.skill.name
            );
            return Ok(SkillOutcome::Waiting);
        }
        if let Some(progress) = ctx.progress {
            print_skill_progress(progress, &skill.skill.name);
        } else {
            print_nested_skill_progress(&skill.skill.name);
        }
        // A completed attempt at this position is the step's completion.
        if flow
            .current_attempt
            .as_ref()
            .is_some_and(|attempt| attempt.outcome.as_deref() == Some("completed"))
        {
            return self.finish().await;
        }
        let step = flow_run::ActiveStep {
            invocation: self.id.clone(),
            version: flow.version,
        };
        let _token = EnvVarGuard::set(flow_run::FLOW_STEP_ENV, &serde_json::to_string(&step)?);
        let mut message = self.message.clone().unwrap_or_default();
        if let Some(direction) = &ctx.direction {
            message.push_str(&format!(
                "\n\nPrevious step feedback or iteration direction:\n{direction}"
            ));
        }
        if skill.repeat.is_some() {
            message.push_str("\n\nRecord this occurrence's decision with `lf flow decide advance \"evidence\"` or `lf flow decide iterate \"next action and proof\"`. If progress is stalled, use `lf flow blocked \"reason, attempted direction, evidence, and question\"`; it opens one Ask running unblock and returns the human's summary. Reassess afterward. Invalid commands return correction feedback; correct the decision here without replaying implementation. A recorded decision is accepted only after this Run succeeds.");
        }
        let mut launch = self.cli.launch_options();
        launch.batch = true;
        launch.interactive = false;
        launch.tui = false;
        launch.ide = false;
        let work = flow.declared_work();
        let result = run_skill_with_journal(
            &self.repo,
            &skill.skill.name,
            ctx.progress.map(|p| p.index),
            || {
                crate::lf::commands::run::run_saved(
                    &skill.skill,
                    Some(&message),
                    &launch,
                    &self.repo,
                    work,
                )?;
                if self.cli.task.is_none() && self.cli.wave.is_none() {
                    commit_skill_work(&self.repo, &skill.skill.name)?;
                }
                Ok(())
            },
        );
        if let Err(error) = result {
            anyhow::bail!("{} Run failed: {error:#}", skill.skill.name);
        }
        self.finish().await
    }

    async fn checkpoint(&self, cursor: &ExecutionCursor) -> Result<()> {
        let version = self
            .store
            .checkpoint_flow(&self.id, self.version(), cursor)
            .await?;
        *self.version.lock().expect("Flow version mutex poisoned") = version;
        Ok(())
    }

    /// An operation is an attempt like a skill launch: its Run row is the
    /// receipt recovery reads, so an interrupted operation blocks for
    /// inspection instead of replaying its side effect.
    async fn run_command(&self, ops: &crate::engine::ConcreteCommand, _ctx: ExecutionContext) -> Result<()> {
        let flow = self.begin().await?;
        let name = ops.item.display_name();
        eprintln!("op: {name}");
        if flow
            .current_attempt
            .as_ref()
            .is_some_and(|attempt| attempt.outcome.as_deref() == Some("completed"))
        {
            return Ok(());
        }
        let capture = crate::run_record::CaptureHandle::begin_with_context(
            crate::run_record::RunSpec {
                harness: "loopflow".into(),
                model: None,
                surface: "operation".into(),
                cwd: self.repo.clone(),
                repo: Some(self.repo.clone()),
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                flow: RunFlowMembership::Step(RunFlowStep::of_flow(&flow)?),
                work: flow.declared_work(),
            },
            &crate::trace::PreparedTurnContext::from_prompts(
                "Loopflow mechanical Flow boundary",
                &name,
            ),
        )?;
        capture.record_input("operation", &name);
        let repo = self.repo.clone();
        let item = ops.item.clone();
        let result = tokio::task::spawn_blocking(move || {
            crate::ops::execute_flow_command(&repo, &item, &NullProgress)
        }).await?;
        capture.finish(if result.is_ok() {
            "completed"
        } else {
            "failed"
        })?;
        result.map_err(|error| anyhow!("op: {name} failed: {error}"))
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

        fs::write(
            temp.path().join(".lf/flows/tend.yaml"),
            "- step: tend/scan-waves\n- xor:\n    router: tend/assess\n    paths:\n      tune:\n        flow: tend/tune\n        description: Adjust the chord\n      silence:\n        description: No-op\n",
        ).unwrap();
        let flow = crate::engine::load_flow("tend", temp.path()).unwrap();

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
