use crate::durable::WorkRef;
use crate::flow::compile_flow;
use crate::flow::output::FlowOutput;
use crate::flow::return_target;
use crate::flow::runner::ExecutionContext;
use crate::flow::runner::ExecutionCursor;
use crate::flow::runner::FlowOutcome;
use crate::flow::runner::FlowRunner;
use crate::flow::runner::SkillExecutor;
use crate::flow::runner::SkillOutcome;
use crate::flow::runner::StepProgress;
use crate::flow::ConcreteSkill;
use crate::flow::ConcreteStep;
use crate::flow::ConcreteXor;
use crate::flow::FlowDefinition;
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::{Cli, Commands, PrCommand};
use crate::ops::flow_process;
use crate::ops::WorkBinding;
use crate::store::SharedStore;
use anyhow::{Context, Result};
use async_trait::async_trait;
use clap::Parser;
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
    if let Some(WorkRef::Task(task)) = binding.map(|binding| &binding.work) {
        block_on(async {
            let store = open_flow_store().await?;
            store.sqlite.require_task_launch(task)?;
            Ok(())
        })?;
    }
    print_pipeline_header(&flow.name, &items);
    let bound_message = binding
        .filter(|binding| !matches!(binding.work, WorkRef::Task(_)))
        .map(|binding| crate::lf::commands::run::bound_message(binding, message));
    execute(
        &flow.name,
        &items,
        ExecutionCursor::default(),
        bound_message.as_deref().or(message),
        cli,
        repo,
        binding,
    )
}

/// `lf --task ISSUE flow __edge`: run the slice of the Task's captured graph
/// that its current edge names, from the route's first step to the next
/// stage. The Task run that chose the edge is this process's parent; it
/// writes the arrival when this process succeeds.
pub fn run_task_edge(cli: &Cli, repo: &Path, binding: Option<&WorkBinding>) -> Result<()> {
    let Some(WorkRef::Task(task)) = binding.map(|binding| &binding.work) else {
        anyhow::bail!("`lf flow __edge` runs a Task's edge; name the Task with --task");
    };
    let store = block_on(open_flow_store())?;
    store.sqlite.require_task_launch(task)?;
    let (graph, start, edge, _) = store
        .sqlite
        .workflow_edge_slice(task)?
        .context("the Task is not on an edge that runs steps; `lf task run` chooses one")?;
    print_pipeline_header(&format!("{} · {edge}", graph.name), &graph.steps);
    execute(&graph.name, &graph.steps, start, None, cli, repo, binding)
}

pub fn show(name: &str, repo: &Path) -> Result<()> {
    let flow = crate::flow::load_authored_flow(name, repo)?;
    let items = compile_flow(&flow, repo)?;
    for line in render_pipeline_lines(&items) {
        println!("{line}");
    }
    Ok(())
}

/// `lf flow list [--json]` — Flow definitions with the topology each would capture.
pub fn list(repo: &Path, json: bool) -> Result<()> {
    let catalog = crate::flow::graph::flow_catalog(repo)?;
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

/// Carry one Task run: start the Task's current edge as a child Flow process,
/// and start it again while a Flow process fails. Each attempt records its own
/// FlowProcess beneath this process. A Flow held for a person or a watcher, an
/// interrupted one, and a launch refused before any Flow started are returned
/// as they ended. `edge` names the edge for messages.
pub fn run_task_edge_for(cli: &Cli, issue: &str, edge: &str, checkout: &Path) -> Result<()> {
    let args = task_run_args(cli, issue, &["flow", "__edge"]);
    let store = block_on(open_flow_store())?;
    let store = &store.sqlite;
    let process =
        journal::current_lf_process_id().context("a Task run requires a registered Process")?;
    let mut attempt = 1;
    loop {
        let mark = store.process_mark()?;
        let status = spawn_task_child(&args, checkout)?;
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
                eprintln!("Edge {edge} failed {attempt} times; this Task run stops here.");
            }
            return Err(exit_of(status).into());
        }
        eprintln!("Edge {edge} failed (attempt {attempt} of {TASK_RUN_ATTEMPTS}); starting it again.");
        attempt += 1;
    }
}

/// Run `skill` for the Task as a headless child, `message` as its first turn,
/// and read the structured answer `output` asks for. `None` when the skill
/// runs with no contract or answers outside it; the caller decides without.
pub(crate) fn run_task_skill(
    cli: &Cli,
    issue: &str,
    checkout: &Path,
    skill: &str,
    message: &str,
    output: Option<&FlowOutput>,
) -> Result<Option<SkillOutcome>> {
    let mut step_cli = cli.process_options();
    step_cli.output_schema = output.map(FlowOutput::schema);
    step_cli.wave = cli.wave.clone();
    let mut args = task_run_args(&step_cli, issue, &["skill", "--", skill]);
    args.push(message.to_owned());
    let store = block_on(open_flow_store())?;
    let store = &store.sqlite;
    let process =
        journal::current_lf_process_id().context("a Task run requires a registered Process")?;
    let mark = store.process_mark()?;
    let status = spawn_task_child(&args, checkout)?;
    if !status.success() {
        return Err(exit_of(status).into());
    }
    let Some(output) = output else {
        return Ok(None);
    };
    let Some(step) = store.child_process_after(&process, mark)? else {
        return Ok(None);
    };
    let Some((_, input, _)) = store.process_input(&step)? else {
        return Ok(None);
    };
    Ok(store
        .input_final_answer(&input)?
        .and_then(|text| answer_value(&text).ok())
        .and_then(|value| output.decode(&value).ok()))
}

fn exit_of(status: std::process::ExitStatus) -> crate::process::CommandExit {
    let code = status.code().and_then(|code| u8::try_from(code).ok());
    crate::process::CommandExit(code.unwrap_or(130))
}

/// Start `lf <args>` in `checkout` and wait for it. An interrupted caller
/// takes the running child with it.
fn spawn_task_child(args: &[String], checkout: &Path) -> Result<std::process::ExitStatus> {
    static ATTEMPT_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    crate::agent::register_interrupt_cleanup(|| {
        let pid = ATTEMPT_PID.load(std::sync::atomic::Ordering::Acquire);
        if pid != 0 {
            crate::platform::kill_process(pid);
        }
    });
    let lf = crate::os_process::resolve_lf_binary()?;
    let mut command = std::process::Command::new(&lf);
    command.args(args).current_dir(checkout);
    crate::transport::configure_child(&mut command);
    let output = crate::terminal_output::TerminalOutput::for_child(&mut command)?;
    let mut child = command.spawn().context("could not start the Task's step")?;
    ATTEMPT_PID.store(child.id(), std::sync::atomic::Ordering::Release);
    let status = child.wait();
    drop(output);
    ATTEMPT_PID.store(0, std::sync::atomic::Ordering::Release);
    status.context("could not wait for the Task's step")
}

/// The child a Task run starts: this launcher's options, the Task, then `tail`.
pub(crate) fn task_run_args(cli: &Cli, issue: &str, tail: &[&str]) -> Vec<String> {
    let mut args = cli.step_args();
    if let Some(wave) = &cli.wave {
        args.extend(["--wave".to_owned(), wave.clone()]);
    }
    args.extend(["--task", issue].map(str::to_owned));
    args.extend(tail.iter().map(|arg| arg.to_string()));
    args
}

/// Drive the Flow in this process from `start`, bracketed by flow journal events.
fn execute(
    flow_name: &str,
    items: &[ConcreteStep],
    start: ExecutionCursor,
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
    binding: Option<&WorkBinding>,
) -> Result<()> {
    let accounts = crate::provider_account::selection::AccountSelection::from_flags_or_env(
        &cli.account,
        &cli.only_account,
    )?;
    report_outcome(block_on(async {
        let running = RunningFlow {
            store: open_flow_store().await?,
            process: journal::current_lf_process_id()
                .context("a Flow requires a registered Process")?,
            flow: flow_name,
            steps: items,
            message,
            cwd: repo,
            launcher: cli,
            position: Mutex::new(start.clone()),
            task: binding.and_then(|binding| match &binding.work {
                WorkRef::Task(id) => Some(id.clone()),
                _ => None,
            }),
        };
        // A chapter rotation moving this checkout's Task excludes new work in it.
        let admission = running
            .store
            .sqlite
            .lock_task_checkouts(&[running.cwd], running.task.as_ref())?;
        // The Flow process's one record of its Flow, written before any step runs.
        running
            .store
            .sqlite
            .record_flow_process(
                &running.process,
                &crate::flow::graph::FlowGraph::new(flow_name, items),
            )
            .context("could not record the Flow; no steps launched")?;
        drop(admission);
        let outcome = drive(&running, start, accounts).await?;
        // A step that completed its Task could not clean up under its own live
        // Flow; the finished Flow can.
        if let (FlowOutcome::Completed, Some(task)) = (&outcome, &running.task) {
            let task = running
                .store
                .get_task(task)
                .await?
                .context("Flow Task disappeared")?;
            crate::ops::task::cleanup_completed_task(&running.store, &task).await?;
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
        FlowOutcome::Staged(stage) => {
            eprintln!("Reached {stage}, where a person takes part; a Task carries the graph on from there.");
            Ok(())
        }
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
/// A Flow process that dies leaves its Processes as history; nothing resumes it.
async fn drive(
    running: &RunningFlow<'_>,
    start: ExecutionCursor,
    accounts: crate::provider_account::selection::AccountSelection,
) -> Result<FlowOutcome> {
    let fields = |extra: LfEventFields| LfEventFields {
        flow: Some(running.flow.to_owned()),
        ..extra
    };
    journal::emit(
        running.cwd,
        LfNode::Flow,
        LfEventType::Started,
        fields(LfEventFields::default()),
    );
    let result = async {
        let _flow_env = EnvVarGuard::set("LOOPFLOW_FLOW_NAME", running.flow);
        let _accounts = accounts.activate()?;
        let mut cursor = start;
        FlowRunner::new(running)
            .run_with_cursor(running.steps, &mut cursor)
            .await
    }
    .await;
    let (event, error) = match &result {
        Ok(FlowOutcome::Completed | FlowOutcome::Staged(_)) => (LfEventType::Completed, None),
        Ok(_) => (LfEventType::Escalated, None),
        Err(error) => (LfEventType::Errored, Some(error.to_string())),
    };
    journal::emit(
        running.cwd,
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
            ConcreteStep::Skill(skill) => match skill.stage() {
                Some(stage) if stage != skill.skill.name => {
                    lines.push(format!("{} [stage {stage}]", skill.skill.name))
                }
                Some(_) => lines.push(format!("{} [stage]", skill.skill.name)),
                None => lines.push(skill.skill.name.clone()),
            },
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
struct RunningFlow<'a> {
    store: SharedStore,
    process: crate::id::LfProcessId,
    flow: &'a str,
    steps: &'a [ConcreteStep],
    message: Option<&'a str>,
    cwd: &'a Path,
    launcher: &'a Cli,
    /// Where the runner stands, as of its last checkpoint.
    position: Mutex<ExecutionCursor>,
    task: Option<crate::durable::TaskId>,
}

/// How a step's process ended, before any answer is read.
enum StepExit {
    Finished,
    /// Held or interrupted: the Flow stops here without an automatic retry.
    Stopped,
}

impl RunningFlow<'_> {
    /// The node the runner stands on and the returns taken to reach it.
    fn location(&self) -> (u32, Vec<Vec<u32>>) {
        let cursor = self.position.lock().expect("Flow position mutex poisoned");
        crate::flow::graph::location(self.steps, &cursor)
            .expect("the runner's position selects a captured node")
    }

    /// The captured step the runner is about to run.
    fn current(&self) -> Option<ConcreteStep> {
        let position = self.position.lock().expect("Flow position mutex poisoned");
        let (body, leaf) = position.current_body(self.steps);
        body.get(leaf.index).cloned()
    }

    /// Record the Process this Flow process's newest child registered, once it has.
    fn record_step(&self, mark: i64) -> Result<Option<crate::id::LfProcessId>> {
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
    ) -> Result<(StepExit, Option<crate::id::LfProcessId>)> {
        // The absolute selected path becomes argv[0] in the child's Process record.
        let mut command = tokio::process::Command::new(crate::os_process::resolve_lf_binary()?);
        command.current_dir(self.cwd);
        command.env(flow_process::FLOW_ID_ENV, self.process.as_str());
        if let Some(fd) =
            crate::ops::cron::accounting::CronExecution::from_env().map(|cron| cron.lock_fd)
        {
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
        if self.launcher.verbose {
            command.arg("--verbose");
        }
        command.args(args);
        let mark = self.store.sqlite.process_mark()?;
        let admission = self
            .store
            .sqlite
            .lock_task_checkouts(&[self.cwd], self.task.as_ref())?;
        crate::transport::configure_child(command.as_std_mut());
        let output = crate::terminal_output::TerminalOutput::for_child(command.as_std_mut())?;
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
        drop(output);
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
                    "this Flow process cannot read the result of {label}; inspect its Process and effects with a compatible lf before launching further work"
                )
            })?;
        let status = status.context("could not execute Flow step")?;
        match status.code() {
            Some(0) => Ok((StepExit::Finished, step)),
            Some(3 | 130) => Ok((StepExit::Stopped, step)),
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
    fn captured(&self, step: Option<&crate::id::LfProcessId>) -> Result<Option<(String, String)>> {
        let Some(step) = step else { return Ok(None) };
        Ok(self
            .store
            .sqlite
            .process_input(step)?
            .map(|(session, input, _)| (session, input)))
    }

    fn turn_events(&self, step: Option<&crate::id::LfProcessId>) -> Result<Vec<serde_json::Value>> {
        Ok(match self.captured(step)? {
            Some((_, input)) => self.store.sqlite.input_events(&input)?,
            None => Vec::new(),
        })
    }

    /// The Session a step's Process opened and the answer its turn returned.
    fn answer(
        &self,
        label: &str,
        step: Option<&crate::id::LfProcessId>,
    ) -> Result<(String, Option<String>)> {
        let step = step.with_context(|| format!("{label} registered no step Process"))?;
        let (session, input) = self.captured(Some(step))?.with_context(|| {
            format!(
                "{label} step Process {} captured no Session input",
                step.as_str()
            )
        })?;
        let answer = self.store.sqlite.input_final_answer(&input)?;
        Ok((session, answer))
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
impl SkillExecutor for &RunningFlow<'_> {
    async fn checkpoint(&self, cursor: &ExecutionCursor) -> Result<()> {
        *self.position.lock().expect("Flow position mutex poisoned") = cursor.clone();
        Ok(())
    }

    async fn run_skill(
        &self,
        skill: &ConcreteSkill,
        ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        crate::ops::linear_observe::sync_repository(&self.store, &self.cwd.to_string_lossy()).await;
        let name = &skill.skill.name;
        print_step_progress(ctx.progress, name);
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
        // spawn carries verbosity for skills, operations, and correction turns.
        step_cli.verbose = false;
        step_cli.output_schema = output.as_ref().map(FlowOutput::schema);
        step_cli.account.clear();
        step_cli.only_account.clear();
        step_cli.isolate = false;
        step_cli.shared = false;
        // The child receives this occurrence's captured source, not a later
        // catalog selection. Keep the file alive through every child/readback.
        let mut input = tempfile::NamedTempFile::new()?;
        serde_json::to_writer(
            &mut input,
            &crate::skills::invocation::SkillInvocation {
                skill: skill.skill.clone(),
                arguments: self.message.unwrap_or_default().to_string(),
            },
        )?;
        step_cli.skill_input = Some(input.path().to_path_buf());
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
                "--output-schema".to_owned(),
                output.schema().to_string(),
                "session".to_owned(),
                "connect".to_owned(),
                conversation,
                "--message".to_owned(),
                format!("The previous final output was invalid: {error}. Return a corrected value using the declared schema; preserve the preceding work.{}", output.instructions()),
            ];
        }
        unreachable!("the correction loop returns or fails")
    }

    /// The operation is its own `lf` command; the Flow process reads how it ended.
    async fn run_command(
        &self,
        ops: &crate::flow::ConcreteCommand,
        _ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        crate::ops::linear_observe::sync_repository(&self.store, &self.cwd.to_string_lossy()).await;
        let label = ops.item.display_name();
        eprintln!("op: {label}");
        // Authored spellings outlive the CLI's; the step runs today's.
        let argv = ops.item.clone().current().argv();
        let parsed = crate::lf::navigation::normalize_args(argv.clone())
            .and_then(Cli::try_parse_from)
            .ok();
        let handoff_checkout = match parsed.and_then(|cli| cli.command) {
            Some(Commands::Pr {
                cmd:
                    Some(PrCommand::Land {
                        local: false,
                        wait_and_fix: false,
                        worktree,
                        ..
                    }),
            }) => Some(crate::ops::land::resolve_repos(self.cwd, worktree.as_deref())?.0),
            _ => None,
        };
        let args: Vec<String> = argv.into_iter().skip(1).collect();
        if let (StepExit::Stopped, _) = self.spawn(&label, &args).await? {
            return Ok(SkillOutcome::Waiting);
        }
        // Only a nonwaiting land hands delivery off. A recent landing in the
        // same checkout says nothing about an unrelated command's result.
        if let Some(checkout) = handoff_checkout {
            if let Some(landing) = self.store.sqlite.pending_landing_at(&checkout)? {
                eprintln!("Landing {landing} is still being watched; the Flow stops here.");
                return Ok(SkillOutcome::Waiting);
            }
        }
        Ok(SkillOutcome::Completed)
    }
}

fn print_step_progress(progress: Option<StepProgress>, name: &str) {
    crate::transport::report(crate::program_status::State::Clear, true, None);
    crate::transport::report(
        crate::program_status::State::Working,
        true,
        progress.map(|p| (p.index * 100 / p.total.max(1)) as u8),
    );
    let position = match progress {
        Some(progress) => format!("{}/{}", progress.index + 1, progress.total),
        None => "*".to_owned(),
    };
    let colors = Colors::new();
    eprintln!(
        "{dim}[{position}]{reset} {bold}{name}{reset}",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
    );
}

#[cfg(test)]
mod tests {
    use super::{render_pipeline_lines, task_run_args};
    use crate::flow::ConcreteStep;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn linear_on_demand_flow_steps_acquire_before_spawning() {
        use crate::flow::runner::{ExecutionContext, ExecutionCursor, SkillExecutor, SkillOutcome};
        use crate::flow::{Command, ConcreteCommand, ConcreteSkill, Skill};
        use crate::lf::Cli;
        use crate::ops::linear::task_planning_tests::{mark_issue_updated, with_planning_task};
        use clap::Parser;
        use std::os::unix::fs::PermissionsExt;
        use std::sync::Mutex;

        with_planning_task(|runtime, fixture, repo, task, state| {
            let _env = crate::test_ambient::EnvGuard::clear(&["LF_BIN"]);
            let child = fixture.directory.path().join("lf-step");
            fs::write(&child, format!(
                "#!/usr/bin/env python3\nimport sqlite3\nwith sqlite3.connect({}) as db:\n    title, = db.execute('SELECT issue_title FROM tasks WHERE id=?', ({},)).fetchone()\nassert title == 'Acquired before step', title\n",
                serde_json::to_string(&fixture.database).unwrap(),
                serde_json::to_string(task.id.as_str()).unwrap(),
            )).unwrap();
            fs::set_permissions(&child, fs::Permissions::from_mode(0o755)).unwrap();
            std::env::set_var("LF_BIN", &child);
            let cli = Cli::try_parse_from(["lf", "--batch"]).unwrap();
            let skill = ConcreteSkill {
                skill: Skill::named("probe"),
                id: None,
                interactive: false,
                returns: None,
                sources: Vec::new(),
            };
            let command = ConcreteCommand {
                item: Command {
                    command: "context".into(),
                    args: Vec::new(),
                },
                sources: Vec::new(),
            };
            let steps = [
                ConcreteStep::Skill(skill.clone()),
                ConcreteStep::Command(command.clone()),
            ];
            let running = super::RunningFlow {
                store: fixture.store.clone(),
                process: crate::id::LfProcessId::new(),
                flow: "probe",
                steps: &steps,
                message: None,
                cwd: repo,
                launcher: &cli,
                position: Mutex::new(ExecutionCursor::default()),
                task: None,
            };
            runtime.block_on(async {
                let conn = rusqlite::Connection::open(&fixture.database).unwrap();
                for index in 0..2 {
                    conn.execute("DELETE FROM linear_team_checks", []).unwrap();
                    conn.execute(
                        "UPDATE tasks SET issue_title='Old title' WHERE id=?1",
                        [task.id.as_str()],
                    )
                    .unwrap();
                    {
                        let mut provider = state.lock().await;
                        provider.requests = 0;
                        provider.issues[0]["title"] = serde_json::json!("Acquired before step");
                        mark_issue_updated(&mut provider.issues[0]);
                    }
                    let ctx = ExecutionContext {
                        progress: None,
                        direction: None,
                    };
                    let outcome = if index == 0 {
                        (&running).run_skill(&skill, ctx).await
                    } else {
                        running.position.lock().unwrap().index = 1;
                        (&running).run_command(&command, ctx).await
                    }
                    .unwrap();
                    assert_eq!(outcome, SkillOutcome::Completed);
                    assert_eq!(state.lock().await.requests, 1);
                }
            });
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

        fs::write(
            temp.path().join(".lf/flows/tend.yaml"),
            "- step: tend/scan-waves\n- xor:\n    router: tend/assess\n    paths:\n      tune:\n        flow: tend/tune\n        description: Adjust the chord\n      silence:\n        description: No-op\n",
        ).unwrap();
        let flow = crate::flow::load_flow("tend", temp.path()).unwrap();

        let items = crate::flow::compile_flow(&flow, temp.path()).unwrap();
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
    fn builtin_task_graphs_compile_and_the_stageless_ones_run_whole() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in ["task-design", "pursue", "queue", "deliver"] {
            let flow = crate::flow::load_flow(name, &repo).unwrap();
            let items = crate::flow::compile_flow(&flow, &repo).unwrap();
            assert!(
                items.iter().all(|item| !matches!(item, ConcreteStep::Skill(skill) if skill.interactive)),
                "{name} has a stage"
            );
        }
        for name in ["feature", "code", "research"] {
            let definition = crate::workflow::load_workflow(name, &repo)
                .unwrap()
                .unwrap_or_else(|| panic!("{name} is a builtin graph"));
            assert!(!definition.nodes.is_empty(), "{name} has stages");
        }
    }

    #[test]
    fn pipeline_lines_mark_stages_hidden_in_xor_paths() {
        let repo = tempdir().unwrap();
        let flows = repo.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(
            flows.join("choice.yaml"),
            "- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - step:\n              id: revise_choice\n              name: implement\n          - interactive:\n              id: review_choice\n              name: review-design\n",
        )
        .unwrap();
        let flow = crate::flow::load_flow("choice", repo.path()).unwrap();
        let items = crate::flow::compile_flow(&flow, repo.path()).unwrap();
        assert!(render_pipeline_lines(&items)
            .iter()
            .any(|line| line.contains("review-design [stage review_choice]")));
    }

    #[test]
    fn flow_steps_run_headless_unless_asked_for_a_terminal() {
        use clap::Parser;
        let mode = |argv: &[&str]| {
            let cli = crate::lf::Cli::try_parse_from(argv).unwrap();
            let args = task_run_args(&cli, "INF-1", &["flow", "__edge"]);
            assert!(args.ends_with(&["--task", "INF-1", "flow", "__edge"].map(String::from)));
            (
                args.contains(&"--batch".to_owned()),
                args.contains(&"--interactive".to_owned()),
            )
        };
        assert_eq!(mode(&["lf", "task", "run", "INF-1"]), (true, false));
        assert_eq!(mode(&["lf", "-b", "task", "run", "INF-1"]), (true, false));
        assert_eq!(mode(&["lf", "-i", "task", "run", "INF-1"]), (false, true));
        let ship = crate::lf::Cli::try_parse_from(["lf", "ship"]).unwrap();
        assert!(ship.step_args().contains(&"--batch".to_owned()));
    }
}
