use crate::durable::WorkRef;
use crate::engine::flow_output::FlowOutput;
use crate::engine::{
    compile_flow, ConcreteSkill, ConcreteStep, ConcreteXor, ExecutionContext, ExecutionCursor,
    Flow, FlowEngine, FlowOutcome, SkillExecutor, SkillOutcome, StepProgress,
};
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::Cli;
use crate::ops::flow_run::{self, FlowStep};
use crate::ops::{NullProgress, WorkBinding};
use crate::store::SharedStore;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// Run a flow: print pipeline header, then execute each skill sequentially.
pub fn run(
    flow: &Flow,
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
    binding: Option<&WorkBinding>,
) -> Result<()> {
    let checkout = if binding.is_none() {
        crate::lf::commands::run::implicit_binding(cli)?
    } else {
        None
    };
    let binding = binding.or(checkout.as_ref());
    let items = compile_flow(flow, repo)?;
    require_autonomous_steps(&items)?;
    if let Some(WorkRef::Task(task)) = binding.map(|binding| &binding.work) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                crate::ops::task::require_task_flow_launch(&open_flow_store().await?, task)
                    .await
                    .map_err(anyhow::Error::from)
            })?;
    }
    print_pipeline_header(&flow.name, &items);
    let bound_message = binding
        .filter(|binding| !matches!(binding.work, WorkRef::Task(_)))
        .map(|binding| crate::lf::commands::run::bound_message(binding, message));
    execute(
        &flow.name,
        &items,
        bound_message.as_deref().or(message),
        cli,
        repo,
        binding,
    )
}

pub(crate) fn require_autonomous_steps(items: &[ConcreteStep]) -> Result<()> {
    for item in items {
        match item {
            ConcreteStep::Skill(skill) if skill.human => anyhow::bail!(
                "Human step {} belongs in the Task conversation; operational Flows cannot launch review Sessions", skill.skill.name
            ),
            ConcreteStep::Xor(branch) => {
                for path in branch.paths.values() {
                    require_autonomous_steps(&path.steps)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn show(name: &str, repo: &Path) -> Result<()> {
    let flow = crate::engine::flow::load_authored_flow(name, repo)?;
    let items = compile_flow(&flow, repo)?;
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

/// Drive the Flow in this process, bracketed by flow journal events.
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
    let accounts = crate::provider_account::lease::AccountSelection::from_flags_or_env(
        &cli.account,
        &cli.only_account,
    )?;
    let task = binding.and_then(|binding| match &binding.work {
        WorkRef::Task(id) => Some(id.clone()),
        _ => None,
    });
    report_outcome(runtime.block_on(async {
        let driver = Driver {
            store: open_flow_store().await?,
            exec: journal::current_exec_id().context("a Flow requires a registered Exec")?,
            flow: flow_name,
            steps: items,
            message,
            cwd: repo,
            launcher: cli,
            position: Mutex::new(ExecutionCursor::default()),
            launched: AtomicU32::new(0),
        };
        let outcome = drive(&driver, accounts).await?;
        // A step that completed its Task could not clean up under its own live
        // Flow; the finished Flow can.
        if let (FlowOutcome::Completed, Some(task)) = (&outcome, &task) {
            let task = driver
                .store
                .get_task(task)
                .await?
                .context("Flow Task disappeared")?;
            crate::ops::task::cleanup_completed_task(&driver.store, &task).await?;
        }
        Ok::<_, anyhow::Error>(outcome)
    })?)
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
            anyhow::bail!("Flow stopped before its last step; inspect its history and effects")
        }
        FlowOutcome::Blocked(reason) => anyhow::bail!("Flow is blocked: {reason}"),
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

/// Run every step from the first until the Flow completes, stops or blocks.
/// A driver that dies leaves its Execs as history; nothing resumes it.
async fn drive(
    driver: &Driver<'_>,
    accounts: crate::provider_account::lease::AccountSelection,
) -> Result<FlowOutcome> {
    let fields = |extra: LfEventFields| LfEventFields {
        flow: Some(driver.flow.to_owned()),
        ..extra
    };
    journal::emit(
        driver.cwd,
        LfNode::Flow,
        LfEventType::Started,
        fields(LfEventFields::default()),
    );
    let result = async {
        let _flow_env = EnvVarGuard::set("LOOPFLOW_FLOW_NAME", driver.flow);
        let _accounts = accounts.activate()?;
        let mut cursor = ExecutionCursor::default();
        FlowEngine::new(driver)
            .run_with_cursor(driver.steps, &mut cursor)
            .await
    }
    .await;
    let (event, error) = match &result {
        Ok(FlowOutcome::Completed) => (LfEventType::Completed, None),
        Ok(_) => (LfEventType::Escalated, None),
        Err(error) => (LfEventType::Errored, Some(error.to_string())),
    };
    journal::emit(
        driver.cwd,
        LfNode::Flow,
        event,
        fields(LfEventFields {
            error,
            ..Default::default()
        }),
    );
    result
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

/// The one Flow executor. It owns the cursor; each step is a child process
/// told its position, and a step's result is read from that child's Exec.
struct Driver<'a> {
    store: SharedStore,
    exec: crate::id::ExecId,
    flow: &'a str,
    steps: &'a [ConcreteStep],
    message: Option<&'a str>,
    cwd: &'a Path,
    launcher: &'a Cli,
    /// Where the engine stands, as of its last checkpoint.
    position: Mutex<ExecutionCursor>,
    launched: AtomicU32,
}

/// How a step's process ended, before any answer is read.
enum StepExit {
    Finished,
    /// Interrupted, or its effect handed to a watcher: the Flow stops here.
    Stopped,
}

impl Driver<'_> {
    /// The next step at the engine's position, with candidates cleared.
    fn step(&self, label: String, output: Option<FlowOutput>) -> FlowStep {
        let mut cursor = self
            .position
            .lock()
            .expect("Flow position mutex poisoned")
            .clone();
        let leaf = cursor.leaf_mut();
        leaf.progress.verdict = None;
        leaf.progress.direction = None;
        leaf.route = None;
        let (key, iterations) = crate::engine::flow_graph::location(self.steps, &cursor)
            .expect("the engine's position selects a captured node");
        FlowStep {
            flow: self.flow.to_owned(),
            seq: self.launched.fetch_add(1, Ordering::SeqCst) + 1,
            label,
            cursor,
            key,
            iterations,
            skill: None,
            output,
            session: None,
        }
    }

    /// The captured step the engine is about to run.
    fn current(&self) -> Option<ConcreteStep> {
        let position = self.position.lock().expect("Flow position mutex poisoned");
        let (body, leaf) = position.current_body(self.steps);
        body.get(leaf.index).cloned()
    }

    async fn spawn(&self, step: &FlowStep, args: &[String], cron: bool) -> Result<StepExit> {
        // The absolute selected path becomes argv[0] in the child's Exec record.
        let mut command =
            tokio::process::Command::new(crate::engine::process::resolve_pinned_lf_binary()?);
        command.current_dir(self.cwd);
        if cron {
            if let Some((id, fd)) = self
                .launcher
                .cron_receipt
                .as_ref()
                .zip(self.launcher.cron_lock_fd)
            {
                command.args(["--__cron-receipt", id, "--__cron-lock-fd", &fd.to_string()]);
                // SAFETY: the cron launcher owns the descriptor throughout spawn;
                // fcntl preserves that exact capability in the step child.
                unsafe {
                    command.pre_exec(move || {
                        if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                            return Err(std::io::Error::last_os_error());
                        }
                        Ok(())
                    });
                }
            }
        }
        command.args(args);
        let status = command.status().await;
        // A new installation can select a newer child. Never read its result
        // through an older schema.
        self.store
            .sqlite
            .validate_current_schema()
            .with_context(|| {
                format!(
                    "this driver cannot read the result of {}; inspect its Exec and effects with a compatible lf before launching further work",
                    step.label
                )
            })?;
        let status = status.context("could not execute Flow step")?;
        match status.code() {
            Some(0) => Ok(StepExit::Finished),
            Some(130) => Ok(StepExit::Stopped),
            Some(code) if code == i32::from(flow_run::HANDED_OFF_EXIT) => Ok(StepExit::Stopped),
            _ => {
                // The step's own conversation says how its turn ended.
                for envelope in self.turn_events(step)?.iter().rev() {
                    let event = &envelope["event"];
                    if event["type"] == "turn_completed" && event["status"] == "interrupted" {
                        return Ok(StepExit::Stopped);
                    }
                    if event["type"] == "error" {
                        anyhow::bail!(
                            "{}: {}: {}",
                            step.label,
                            event["code"].as_str().unwrap_or("provider error"),
                            event["message"].as_str().unwrap_or("provider turn failed")
                        );
                    }
                }
                anyhow::bail!("{} process exited with {status}", step.label)
            }
        }
    }

    /// The conversation and input a step's Exec captured; none for an operation.
    fn captured(&self, step: &FlowStep) -> Result<Option<(String, String)>> {
        let Some(child) = self.store.sqlite.flow_step_exec(&self.exec, step.seq)? else {
            return Ok(None);
        };
        Ok(self
            .store
            .sqlite
            .exec_input(&child.id)?
            .map(|(session, input, _)| (session, input)))
    }

    fn turn_events(&self, step: &FlowStep) -> Result<Vec<serde_json::Value>> {
        Ok(match self.captured(step)? {
            Some((_, input)) => self.store.sqlite.input_events(&input)?,
            None => Vec::new(),
        })
    }

    /// The Session a step's Exec opened and the answer its turn returned.
    fn answer(&self, step: &FlowStep) -> Result<(String, Option<String>)> {
        let (session, input) = self
            .captured(step)?
            .with_context(|| format!("{} captured no Session input", step.label))?;
        let answer = self.store.sqlite.input_final_answer(&input)?;
        Ok((session, answer.map(|answer| answer.text)))
    }
}

#[async_trait]
impl SkillExecutor for &Driver<'_> {
    async fn checkpoint(&self, cursor: &ExecutionCursor) -> Result<()> {
        *self.position.lock().expect("Flow position mutex poisoned") = cursor.clone();
        Ok(())
    }

    async fn run_skill(
        &self,
        skill: &ConcreteSkill,
        ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        anyhow::ensure!(!skill.human, "Human review belongs in the Task conversation; operational Flows cannot launch review Sessions");
        if let Some(progress) = ctx.progress {
            print_skill_progress(progress, &skill.skill.name);
        } else {
            print_nested_skill_progress(&skill.skill.name);
        }
        let current = self.current();
        let output = current.as_ref().and_then(FlowOutput::for_step);
        let mut message = self.message.unwrap_or_default().to_owned();
        if let Some(repeat) = &skill.repeat {
            let edge = skill.id.as_deref().expect("repeat occurrence has an id");
            let traversals = self
                .position
                .lock()
                .expect("Flow position mutex poisoned")
                .leaf()
                .progress
                .repeats
                .get(edge)
                .copied()
                .unwrap_or(0);
            message.push_str(&format!("\n\nDecision occurrence {edge}: pass {}. The backward edge returns to {}. Compare the preceding pass's intended progress with its observed results; new evidence counts as progress. Missing prior evidence is an evidence gap, not proof of no progress.", u64::from(traversals) + 1, repeat.from));
        }
        if let Some(direction) = &ctx.direction {
            message.push_str(&format!(
                "\n\nPrevious step feedback or iteration direction:\n{direction}"
            ));
        }
        if let Some(instructions) = current.as_ref().and_then(FlowOutput::for_step_instructions) {
            message.push_str(&instructions);
        }
        let mut step_cli = self.launcher.exec_options();
        step_cli.account.clear();
        step_cli.only_account.clear();
        step_cli.isolate = false;
        step_cli.shared = false;
        // The step runs the skill this driver compiled. Its argv carries that
        // skill only when a lookup by name would find something else.
        let compiled = (crate::engine::flow::load_skill(&skill.skill.name, self.cwd)
            .ok()
            .as_ref()
            != Some(&skill.skill))
        .then(|| skill.skill.clone());
        let mut session = None;
        // An invalid answer is corrected in the same conversation, twice at most.
        for turn in 1.. {
            let mut step = self.step(skill.skill.name.clone(), output.clone());
            step.skill = compiled.clone();
            step.session = session.take();
            let mut args = step_cli.step_args();
            args.extend([
                "--__cwd".to_owned(),
                self.cwd.to_string_lossy().into_owned(),
            ]);
            args.extend([
                flow_run::FLOW_STEP_ARG.to_owned(),
                step.arg()?,
                "skill".to_owned(),
                skill.skill.name.clone(),
            ]);
            if !message.trim().is_empty() {
                args.push(message.clone());
            }
            if let StepExit::Stopped = self.spawn(&step, &args, false).await? {
                return Ok(SkillOutcome::Waiting);
            }
            let Some(output) = &output else {
                return Ok(SkillOutcome::Completed);
            };
            let (conversation, answer) = self.answer(&step)?;
            let error = match answer {
                None => "final output is absent".to_owned(),
                Some(text) => match serde_json::from_str(&text)
                    .map_err(|_| "final output must be JSON".to_owned())
                    .and_then(|value| output.decode(&value))
                {
                    Ok(outcome) => return Ok(outcome),
                    Err(error) => error,
                },
            };
            anyhow::ensure!(
                turn < 3,
                "structured output validation exhausted after {turn} successful turns: {error}"
            );
            session = Some(conversation);
            message = format!("The previous final output was invalid: {error}. Return a corrected value using the declared schema; preserve the preceding work.{}", output.instructions());
        }
        unreachable!("the correction loop returns or fails")
    }

    /// The child owns the effect; the driver reads only how it ended.
    async fn run_command(
        &self,
        ops: &crate::engine::ConcreteCommand,
        _ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        eprintln!("op: {}", ops.item.display_name());
        let step = self.step(ops.item.display_name(), None);
        let mut args = vec![flow_run::FLOW_STEP_COMMAND.to_owned(), step.arg()?];
        args.extend(ops.item.argv().into_iter().skip(1));
        Ok(match self.spawn(&step, &args, true).await? {
            StepExit::Finished => SkillOutcome::Completed,
            StepExit::Stopped => SkillOutcome::Waiting,
        })
    }
}

/// Run one operation step in its own process. A landing still being watched
/// leaves with `HANDED_OFF_EXIT`: its effect is recorded and the Flow stops.
pub fn execute_step(command: &[String], cli: &Cli) -> Result<()> {
    let cron = cli
        .cron_receipt
        .as_ref()
        .zip(cli.cron_lock_fd)
        .map(|(id, fd)| crate::ops::cron::accounting::CronExecution {
            receipt_id: id.clone(),
            lock_fd: fd,
        });
    let (name, args) = command
        .split_first()
        .ok_or_else(|| anyhow!("Flow step names no operation"))?;
    // Decoding applies the same command normalization a compiled Flow gets.
    let item: crate::engine::flow::Command =
        serde_json::from_value(serde_json::json!({"command": name, "args": args}))?;
    let cwd = crate::repo::working_directory()?;
    block_on_store(|store| async move {
        if let Some(crate::ops::WorkBinding {
            work: WorkRef::Task(task),
            ..
        }) = crate::ops::resolve_execution_binding(&store, &cwd).await?
        {
            store.sqlite.mark_task_started(&task)?;
        }
        let result = tokio::task::spawn_blocking(move || {
            crate::ops::execute_flow_command_with_cron(&cwd, &item, &NullProgress, cron.as_ref())
        })
        .await
        .context("Flow operation worker failed")?;
        // Interrupt cleanup can kill the operation and wake this waiter. Its
        // exit is not evidence that the external effect failed or completed.
        crate::engine::agent::wait_for_interrupt_cleanup();
        let Some(landing) = result? else {
            return Ok(());
        };
        use crate::pr_landing::PrLandingState;
        match store
            .get_pr_landing(&landing)
            .await?
            .map(|landing| landing.state)
        {
            None | Some(PrLandingState::Merged) => Ok(()),
            Some(PrLandingState::Closed) => anyhow::bail!("PR closed without merging"),
            Some(
                PrLandingState::Blocked | PrLandingState::Watching | PrLandingState::Repairing,
            ) => {
                eprintln!("Landing {landing} is still being watched; the Flow stops here.");
                Err(crate::exec::CommandExit(flow_run::HANDED_OFF_EXIT).into())
            }
        }
    })
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
    use crate::engine::ConcreteStep;
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

        let items = crate::engine::compile_flow(&flow, temp.path()).unwrap();
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
    fn builtin_task_flows_launch_as_operational_work() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in ["feature", "code", "task-design", "pursue", "queue", "ship"] {
            let flow = crate::engine::load_flow(name, &repo).unwrap();
            let items = crate::engine::compile_flow(&flow, &repo).unwrap();
            super::require_autonomous_steps(&items)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        }
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
        let items = crate::engine::compile_flow(&flow, repo.path()).unwrap();
        assert!(render_pipeline_lines(&items)
            .iter()
            .any(|line| line.contains("review-design [review:review_choice]")));
    }
}
