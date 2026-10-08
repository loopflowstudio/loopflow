use crate::durable::WorkRef;
use crate::engine::flow::return_target;
use crate::engine::flow_output::FlowOutput;
use crate::engine::{
    compile_flow, ConcreteSkill, ConcreteStep, ConcreteXor, ExecutionContext, ExecutionCursor,
    FlowDefinition, FlowEngine, FlowOutcome, SkillExecutor, SkillOutcome, StepProgress,
};
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::Cli;
use crate::ops::flow_process;
use crate::ops::WorkBinding;
use crate::store::SharedStore;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::Path;
use std::sync::Mutex;

/// Run a flow: print pipeline header, then execute each skill sequentially.
pub fn run(
    flow: &FlowDefinition,
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
        block_on(async {
            Ok(crate::ops::task::require_task_flow_launch(&open_flow_store().await?, task).await?)
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

/// `lf flow list [--json]` — Flow definitions with the topology each would capture.
pub fn list(repo: &Path, json: bool) -> Result<()> {
    let catalog = crate::engine::flow_graph::flow_catalog(repo)?;
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

/// How many times one `lf task run` starts its Flow before giving up.
const TASK_RUN_ATTEMPTS: u32 = 3;

/// Carry one Task run: start `flow` as the plain `lf --task ISSUE run FLOW`
/// child, and again while a Flow process fails. Each attempt is its own FlowProcess
/// beneath this process. A Flow held for a person or a watcher, an interrupted
/// one, and a launch refused before any Flow started are returned as they
/// ended.
pub fn run_for_task(cli: &Cli, issue: &str, flow: &str) -> Result<()> {
    let mut args = cli.step_args();
    if !cli.batch {
        args.retain(|arg| arg != "--batch");
    }
    if let Some(wave) = &cli.wave {
        args.extend(["--wave".to_owned(), wave.clone()]);
    }
    args.extend(["--task", issue, "run", flow].map(str::to_owned));
    let lf = crate::engine::process::resolve_pinned_lf_binary()?;
    let store = block_on(open_flow_store())?;
    let store = &store.sqlite;
    let process =
        journal::current_process_lfid().context("a Task run requires a registered Process")?;
    // An interrupted Task run takes its running attempt with it.
    static ATTEMPT_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    crate::engine::agent::register_interrupt_cleanup(|| {
        let pid = ATTEMPT_PID.load(std::sync::atomic::Ordering::Acquire);
        if pid != 0 {
            crate::engine::platform::kill_process(pid);
        }
    });
    let mut attempt = 1;
    loop {
        let mark = store.process_mark()?;
        let mut child = std::process::Command::new(&lf)
            .args(&args)
            .spawn()
            .context("could not start the Task's Flow")?;
        ATTEMPT_PID.store(child.id(), std::sync::atomic::Ordering::Release);
        let status = child.wait();
        ATTEMPT_PID.store(0, std::sync::atomic::Ordering::Release);
        let status = status.context("could not wait for the Task's Flow")?;
        if status.success() {
            return Ok(());
        }
        // Only a Flow process that failed is started again.
        let failed = status.code() == Some(1)
            && store
                .child_process_after(&process, mark)?
                .is_some_and(|child| matches!(store.flow_process(child.as_str()), Ok(Some(_))));
        if !failed || attempt == TASK_RUN_ATTEMPTS {
            if failed {
                eprintln!("Flow {flow} failed {attempt} times; this Task run stops here.");
            }
            let code = status.code().and_then(|code| u8::try_from(code).ok());
            return Err(crate::process::CommandExit(code.unwrap_or(130)).into());
        }
        eprintln!(
            "Flow {flow} failed (attempt {attempt} of {TASK_RUN_ATTEMPTS}); starting it again."
        );
        attempt += 1;
    }
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
    let accounts = crate::provider_account::selection::AccountSelection::from_flags_or_env(
        &cli.account,
        &cli.only_account,
    )?;
    let task = binding.and_then(|binding| match &binding.work {
        WorkRef::Task(id) => Some(id.clone()),
        _ => None,
    });
    report_outcome(block_on(async {
        let driver = Driver {
            store: open_flow_store().await?,
            process: journal::current_process_lfid()
                .context("a Flow requires a registered Process")?,
            flow: flow_name,
            steps: items,
            message,
            cwd: repo,
            launcher: cli,
            position: Mutex::new(ExecutionCursor::default()),
            task: task.clone(),
            steers: Mutex::default(),
        };
        // A chapter rotation moving this checkout's Task excludes new work in it.
        let admission = driver
            .store
            .sqlite
            .lock_task_checkouts(&[driver.cwd], driver.task.as_ref())?;
        // The driver's one record of its Flow, written before any step runs.
        driver.store.sqlite.record_flow_process(
            &driver.process,
            flow_name,
            &crate::engine::flow_graph::FlowGraph::new(flow_name, items),
        )?;
        // A conversation the Flow opens also starts its Task; a checkout the
        // registry spells differently must not refuse the launch.
        if let Some(task) = &task {
            if let Err(error) = driver.store.sqlite.mark_task_started(task) {
                tracing::warn!(%error, "Flow launch did not record its Task as started");
            }
        }
        drop(admission);
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
        Ok(outcome)
    })?)
}

/// Run `future` to completion from this synchronous command.
fn block_on<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(future)
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
        FlowOutcome::Waiting => Err(crate::process::FlowHeld(
            "Flow stopped before its last step; inspect its history and effects".into(),
        )
        .into()),
        FlowOutcome::Blocked(reason) => {
            Err(crate::process::FlowHeld(format!("Flow is blocked: {reason}")).into())
        }
    }
}

/// Run every step from the first until the Flow completes, stops or blocks.
/// A driver that dies leaves its Processes as history; nothing resumes it.
async fn drive(
    driver: &Driver<'_>,
    accounts: crate::provider_account::selection::AccountSelection,
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

/// The one Flow executor. It owns the cursor and the Flow's record. Each step
/// is an ordinary command run as a child process; its result is read from the
/// Process that child registered.
struct Driver<'a> {
    store: SharedStore,
    process: crate::id::ProcessLfid,
    flow: &'a str,
    steps: &'a [ConcreteStep],
    message: Option<&'a str>,
    cwd: &'a Path,
    launcher: &'a Cli,
    /// Where the engine stands, as of its last checkpoint.
    position: Mutex<ExecutionCursor>,
    task: Option<crate::durable::TaskId>,
    /// The Task's newest steer when each node last started, by node key.
    steers: Mutex<std::collections::HashMap<u32, i64>>,
}

/// How a step's process ended, before any answer is read.
enum StepExit {
    Finished,
    /// Interrupted: the Flow stops here.
    Stopped,
}

impl Driver<'_> {
    /// The node the engine stands on and the returns taken to reach it.
    fn location(&self) -> (u32, Vec<Vec<u32>>) {
        let cursor = self.position.lock().expect("Flow position mutex poisoned");
        crate::engine::flow_graph::location(self.steps, &cursor)
            .expect("the engine's position selects a captured node")
    }

    /// The captured step the engine is about to run.
    fn current(&self) -> Option<ConcreteStep> {
        let position = self.position.lock().expect("Flow position mutex poisoned");
        let (body, leaf) = position.current_body(self.steps);
        body.get(leaf.index).cloned()
    }

    /// Record the Process this driver's newest child registered, once it has.
    fn record_step(&self, mark: i64) -> Result<Option<crate::id::ProcessLfid>> {
        let Some(step) = self.store.sqlite.child_process_after(&self.process, mark)? else {
            return Ok(None);
        };
        let (key, iterations) = self.location();
        self.store
            .sqlite
            .record_flow_step(&self.process, &step, key, &iterations)?;
        Ok(Some(step))
    }

    /// Run `lf <args>` as a child and record its Process as this Flow's next step.
    async fn spawn(
        &self,
        label: &str,
        args: &[String],
    ) -> Result<(StepExit, Option<crate::id::ProcessLfid>)> {
        // The absolute selected path becomes argv[0] in the child's Process record.
        let mut command =
            tokio::process::Command::new(crate::engine::process::resolve_pinned_lf_binary()?);
        command.current_dir(self.cwd);
        command.env(flow_process::FLOW_ID_ENV, self.process.as_str());
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
        command.args(args);
        let mark = self.store.sqlite.process_mark()?;
        let admission = self
            .store
            .sqlite
            .lock_task_checkouts(&[self.cwd], self.task.as_ref())?;
        let mut child = command.spawn().context("could not execute Flow step")?;
        drop(admission);
        let mut step = None;
        let status = loop {
            tokio::select! {
                status = child.wait() => break status,
                // A read can fail while a newer child migrates; the exit retries it.
                () = tokio::time::sleep(std::time::Duration::from_millis(10)), if step.is_none() => {
                    step = self.record_step(mark).unwrap_or(None);
                }
            }
        };
        // The step ran whatever comes next, so its row is attempted first.
        if step.is_none() {
            step = self.record_step(mark).unwrap_or(None);
        }
        // A new installation can select a newer child. Never read its result
        // through an older schema.
        self.store
            .sqlite
            .validate_current_schema()
            .with_context(|| {
                format!(
                    "this driver cannot read the result of {label}; inspect its Process and effects with a compatible lf before launching further work"
                )
            })?;
        let status = status.context("could not execute Flow step")?;
        match status.code() {
            Some(0) => Ok((StepExit::Finished, step)),
            Some(130) => Ok((StepExit::Stopped, step)),
            _ => {
                // The step's own conversation says how its turn ended.
                for envelope in self.turn_events(step.as_ref())?.iter().rev() {
                    let event = &envelope["event"];
                    if event["type"] == "turn_completed" && event["status"] == "interrupted" {
                        return Ok((StepExit::Stopped, step));
                    }
                    if event["type"] == "error" {
                        anyhow::bail!(
                            "{label}: {}: {}",
                            event["code"].as_str().unwrap_or("provider error"),
                            event["message"].as_str().unwrap_or("provider turn failed")
                        );
                    }
                }
                anyhow::bail!("{label} process exited with {status}")
            }
        }
    }

    /// The conversation and input a step's Process captured; none for an operation.
    fn captured(&self, step: Option<&crate::id::ProcessLfid>) -> Result<Option<(String, String)>> {
        let Some(step) = step else { return Ok(None) };
        Ok(self
            .store
            .sqlite
            .process_input(step)?
            .map(|(session, input, _)| (session, input)))
    }

    fn turn_events(&self, step: Option<&crate::id::ProcessLfid>) -> Result<Vec<serde_json::Value>> {
        Ok(match self.captured(step)? {
            Some((_, input)) => self.store.sqlite.input_events(&input)?,
            None => Vec::new(),
        })
    }

    /// The Session a step's Process opened and the answer its turn returned.
    fn answer(
        &self,
        label: &str,
        step: Option<&crate::id::ProcessLfid>,
    ) -> Result<(String, Option<String>)> {
        let (session, input) = self
            .captured(step)?
            .with_context(|| format!("{label} captured no Session input"))?;
        let answer = self.store.sqlite.input_final_answer(&input)?;
        Ok((session, answer.map(|answer| answer.text)))
    }
}

/// The JSON value a final answer carries: the answer itself, or the object
/// inside the prose or code fence a provider wrapped it in.
fn answer_value(text: &str) -> Result<serde_json::Value, String> {
    let invalid = || "final output must be JSON".to_owned();
    serde_json::from_str(text.trim()).or_else(|_| {
        let start = text.find('{').ok_or_else(invalid)?;
        let end = text.rfind('}').ok_or_else(invalid)?;
        serde_json::from_str(text.get(start..=end).ok_or_else(invalid)?).map_err(|_| invalid())
    })
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
        let name = &skill.skill.name;
        if let Some(progress) = ctx.progress {
            print_skill_progress(progress, name);
        } else {
            print_nested_skill_progress(name);
        }
        let current = self.current();
        let output = current.as_ref().and_then(FlowOutput::for_step);
        let mut message = self.message.unwrap_or_default().to_owned();
        if skill.returns.is_some() {
            let position = self.position.lock().expect("Flow position mutex poisoned");
            let (body, leaf) = position.current_body(self.steps);
            let traversals = leaf.progress.repeats.get(&leaf.index.to_string());
            let target = match return_target(body, leaf.index).map(|target| &body[target]) {
                Some(ConcreteStep::Skill(target)) => target.skill.name.as_str(),
                _ => "an earlier step",
            };
            message.push_str(&format!("\n\nDecision pass {}. The backward edge returns to {target}. Compare the preceding pass's intended progress with its observed results; new evidence counts as progress. Missing prior evidence is an evidence gap, not proof of no progress.", u64::from(traversals.copied().unwrap_or(0)) + 1));
        }
        if let Some(direction) = &ctx.direction {
            message.push_str(&format!(
                "\n\nPrevious step feedback or iteration direction:\n{direction}"
            ));
        }
        if let Some(instructions) = current.as_ref().and_then(FlowOutput::for_step_instructions) {
            message.push_str(&instructions);
        }
        let mut step_cli = self.launcher.process_options();
        step_cli.account.clear();
        step_cli.only_account.clear();
        step_cli.isolate = false;
        step_cli.shared = false;
        // A node that runs again is spared the Task direction its earlier run
        // was already given.
        if let Some(task) = &self.task {
            let newest = self
                .store
                .task_steers(task)
                .await?
                .last()
                .map_or(0, |steer| steer.id);
            let (node, _) = self.location();
            let seen = self
                .steers
                .lock()
                .expect("Flow steers mutex poisoned")
                .insert(node, newest);
            step_cli.steers_after = seen.max(step_cli.steers_after);
        }
        // The step looks its skill up by name. An agent the Flow names for
        // this occurrence is the one option that travels, as `--model`.
        if step_cli.model.is_none() {
            let authored = crate::engine::flow::load_skill(name, self.cwd).ok();
            if authored.and_then(|authored| authored.agent) != skill.skill.agent {
                step_cli.model = skill.skill.agent.clone();
            }
        }
        // The step is the command a person would type: `lf -b skill <name> [message]`.
        let mut args = step_cli.step_args();
        args.extend(["skill".to_owned(), name.clone()]);
        if !message.trim().is_empty() {
            args.push(message);
        }
        // An invalid answer is corrected in the same conversation, twice at most.
        for turn in 1.. {
            let (exit, step) = self.spawn(name, &args).await?;
            if let StepExit::Stopped = exit {
                return Ok(SkillOutcome::Waiting);
            }
            let Some(output) = &output else {
                return Ok(SkillOutcome::Completed);
            };
            let (conversation, answer) = self.answer(name, step.as_ref())?;
            let error = match answer {
                None => "final output is absent".to_owned(),
                Some(text) => match answer_value(&text).and_then(|value| output.decode(&value)) {
                    Ok(outcome) => return Ok(outcome),
                    Err(error) => error,
                },
            };
            anyhow::ensure!(
                turn < 3,
                "structured output validation exhausted after {turn} successful turns: {error}"
            );
            args = vec![
                "--batch".to_owned(),
                "session".to_owned(),
                "resume".to_owned(),
                conversation,
                format!("The previous final output was invalid: {error}. Return a corrected value using the declared schema; preserve the preceding work.{}", output.instructions()),
            ];
        }
        unreachable!("the correction loop returns or fails")
    }

    /// The operation is its own `lf` command; the driver reads how it ended.
    async fn run_command(
        &self,
        ops: &crate::engine::ConcreteCommand,
        _ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        let label = ops.item.display_name();
        eprintln!("op: {label}");
        let started = crate::store::rows::now_unix();
        // Authored spellings outlive the CLI's; the step runs today's.
        let args: Vec<String> = ops
            .item
            .clone()
            .current()
            .argv()
            .into_iter()
            .skip(1)
            .collect();
        if let (StepExit::Stopped, _) = self.spawn(&label, &args).await? {
            return Ok(SkillOutcome::Waiting);
        }
        // A landing the step left to its watcher has not delivered yet. Its
        // effect is recorded; the Flow stops here and neither failed.
        if let Some(landing) = self.store.sqlite.watched_landing_at(self.cwd, started)? {
            eprintln!("Landing {landing} is still being watched; the Flow stops here.");
            return Ok(SkillOutcome::Waiting);
        }
        Ok(SkillOutcome::Completed)
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
        for name in ["task-design", "pursue", "queue", "ship"] {
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
