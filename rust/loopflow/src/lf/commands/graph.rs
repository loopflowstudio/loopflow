use crate::definition::{resolve_definition, Target};
use crate::durable::WorkRef;
use crate::graph::compile_graph;
use crate::graph::output::GraphOutput;
use crate::graph::return_target;
use crate::graph::runner::ExecutionContext;
use crate::graph::runner::ExecutionCursor;
use crate::graph::runner::GraphOutcome;
use crate::graph::runner::GraphRunner;
use crate::graph::runner::SkillExecutor;
use crate::graph::runner::SkillOutcome;
use crate::graph::runner::StepProgress;
use crate::graph::ConcreteSkill;
use crate::graph::ConcreteStep;
use crate::graph::ConcreteXor;
use crate::graph::GraphDefinition;
use crate::id::LfProcessId;
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::{Cli, Commands, PrCommand};
use crate::ops::graph_process;
use crate::ops::WorkBinding;
use crate::store::sqlite::SqliteStore;
use crate::store::SharedStore;
use crate::work::task::Task;
use anyhow::{Context, Result};
use async_trait::async_trait;
use clap::Parser;
use std::path::Path;
use std::sync::Mutex;

/// Run a graph: print pipeline header, then execute each skill sequentially.
pub fn run(
    graph: &GraphDefinition,
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
    let items = compile_graph(graph, repo)?;
    require_autonomous_steps(&items)?;
    if let Some(WorkRef::Task(task)) = binding.map(|binding| &binding.work) {
        block_on(async {
            let store = open_graph_store().await?;
            store.sqlite.require_task_launch(task)?;
            Ok(())
        })?;
    }
    print_pipeline_header(&graph.name, &items);
    let bound_message = binding
        .filter(|binding| !matches!(binding.work, WorkRef::Task(_)))
        .map(|binding| crate::lf::commands::run::bound_message(binding, message));
    execute(
        &graph.name,
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
                "Human step {} belongs in the Task conversation; operational Graphs cannot launch review Sessions", skill.skill.name
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
    let graph = crate::graph::load_authored_graph(name, repo)?;
    let items = compile_graph(&graph, repo)?;
    for line in render_pipeline_lines(&items) {
        println!("{line}");
    }
    Ok(())
}

/// `lf graph list [--json]` — Graph definitions with the topology each would capture.
pub fn list(repo: &Path, json: bool) -> Result<()> {
    let catalog = crate::graph::compiled::graph_catalog(repo)?;
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

/// How many times one `lf task run` starts its Graph before giving up.
const TASK_RUN_ATTEMPTS: u32 = 3;

/// Carry one Task run: resolve the edge to an explicit skill or Graph child,
/// and start it again while a Graph process fails. Each Graph attempt records its
/// own GraphProcess beneath this process; a single skill remains a skill launch. A Graph held for a person or a watcher, an interrupted
/// one, and a launch refused before any Graph started are returned as they
/// ended.
///
/// A bare run inside its execution owner lets its launcher go once the first
/// attempt has registered, and carries on with its output in that attempt's log.
pub fn run_for_task(cli: &Cli, task: &Task, graph: &str) -> Result<()> {
    let checkout = task.worktree()?;
    let owner = match resolve_definition(checkout, graph, None)? {
        Target::Graph(_) => "graph",
        Target::Skill(_) => "skill",
        Target::Command(_) | Target::Xor(_) => {
            unreachable!("named definitions are skills or Graphs")
        }
    };
    let args = task_run_args(cli, task.id.as_str(), owner, graph);
    let lf = crate::os_process::resolve_lf_binary()?;
    let store = block_on(open_graph_store())?;
    let store = &store.sqlite;
    let process =
        journal::current_lf_process_id().context("a Task run requires a registered Process")?;
    // An interrupted Task run takes its running attempt with it.
    static ATTEMPT_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    crate::agent::register_interrupt_cleanup(|| {
        let pid = ATTEMPT_PID.load(std::sync::atomic::Ordering::Acquire);
        if pid != 0 {
            crate::platform::kill_process(pid);
        }
    });
    let letting_go = !cli.batch && !cli.interactive && crate::transport::current().is_some();
    let mut released = false;
    let mut attempt = 1;
    loop {
        let mark = store.process_mark()?;
        let mut command = std::process::Command::new(&lf);
        command.args(&args).current_dir(checkout);
        crate::transport::configure_child(&mut command);
        let log = letting_go
            .then(|| PendingLog::open(&process, attempt))
            .transpose()?;
        let output = match &log {
            Some(log) => {
                log.configure_child(&mut command)?;
                None
            }
            None => crate::terminal_output::TerminalOutput::for_child(&mut command)?,
        };
        let mut child = command
            .spawn()
            .context("could not start the Task's Graph")?;
        ATTEMPT_PID.store(child.id(), std::sync::atomic::Ordering::Release);
        if let Some(mut log) = log {
            match registration(store, &process, mark, owner, &mut child)? {
                Some(registered) => {
                    log.settle(&registered)?;
                    if !released {
                        let noun = if owner == "graph" {
                            "Graph process"
                        } else {
                            "Process"
                        };
                        println!(
                            "{} · {graph} running as {noun} {registered}\n  follow: lf task status {}\n  log:    {}",
                            task.identifier,
                            task.identifier,
                            log.path.display()
                        );
                        release_launcher(&log.file)?;
                        released = true;
                    }
                }
                None => log.replay(),
            }
        }
        let status = child.wait();
        drop(output);
        ATTEMPT_PID.store(0, std::sync::atomic::Ordering::Release);
        let status = status.context("could not wait for the Task's Graph")?;
        if status.success() {
            return Ok(());
        }
        // Only a Graph process that failed is started again.
        let failed = status.code() == Some(1) && attempt_graph(store, &process, mark)?.is_some();
        if !failed || attempt == TASK_RUN_ATTEMPTS {
            if failed {
                eprintln!("Graph {graph} failed {attempt} times; this Task run stops here.");
            }
            let code = status.code().and_then(|code| u8::try_from(code).ok());
            return Err(crate::process::CommandExit(code.unwrap_or(130)).into());
        }
        eprintln!(
            "Graph {graph} failed (attempt {attempt} of {TASK_RUN_ATTEMPTS}); starting it again."
        );
        attempt += 1;
    }
}

/// Where a Task run that let its launcher go keeps its Graph or skill Process's output.
pub fn graph_log_path(id: &str) -> std::path::PathBuf {
    crate::store::lf_home_dir()
        .join("logs")
        .join(format!("graph-{id}.log"))
}

/// The attempt's output until it registers; it then becomes that Process's log.
#[derive(Debug)]
struct PendingLog {
    file: std::fs::File,
    path: std::path::PathBuf,
}

impl PendingLog {
    fn open(process: &LfProcessId, attempt: u32) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let path = graph_log_path(&format!("pending-{process}-{attempt}"));
        std::fs::create_dir_all(path.parent().expect("graph logs live in a directory"))?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .with_context(|| format!("could not open the Graph log {}", path.display()))?;
        Ok(Self { file, path })
    }

    fn configure_child(&self, command: &mut std::process::Command) -> Result<()> {
        command
            .stdin(std::process::Stdio::null())
            .stdout(self.file.try_clone()?)
            .stderr(self.file.try_clone()?);
        Ok(())
    }

    /// Name the log for the registered Process; open descriptors follow the rename.
    fn settle(&mut self, registered: &LfProcessId) -> Result<()> {
        let path = graph_log_path(registered.as_str());
        std::fs::rename(&self.path, &path)
            .with_context(|| format!("could not name the Graph log {}", path.display()))?;
        self.path = path;
        Ok(())
    }

    /// An attempt that ended before registering tells its launcher why.
    fn replay(mut self) {
        use std::io::{Read, Seek, Write};
        let mut bytes = Vec::new();
        if self.file.rewind().is_ok() && self.file.read_to_end(&mut bytes).is_ok() {
            let _ = std::io::stderr().write_all(&bytes);
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The attempt's child once it has recorded a GraphProcess.
fn attempt_graph(
    store: &SqliteStore,
    process: &LfProcessId,
    mark: i64,
) -> Result<Option<LfProcessId>> {
    Ok(store
        .child_process_after(process, mark)?
        .filter(|child| matches!(store.graph_process(child.as_str()), Ok(Some(_)))))
}

/// Wait until the attempt registers — a Graph its GraphProcess, a skill its
/// Process — or ends first. No deadline: the child's own exit bounds it.
fn registration(
    store: &SqliteStore,
    process: &LfProcessId,
    mark: i64,
    owner: &str,
    child: &mut std::process::Child,
) -> Result<Option<LfProcessId>> {
    loop {
        let registered = if owner == "graph" {
            attempt_graph(store, process, mark)?
        } else {
            store.child_process_after(process, mark)?
        };
        if registered.is_some() {
            return Ok(registered);
        }
        if child.try_wait()?.is_some() {
            return Ok(None);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Hand the launcher its last lines, end its view, and keep this run's own
/// output in the log from here on.
fn release_launcher(log: &std::fs::File) -> Result<()> {
    use std::io::Write;
    std::io::stdout().flush()?;
    std::io::stderr().flush()?;
    crate::transport::detach_viewers()?;
    crate::terminal_output::redirect(log, 1)?;
    crate::terminal_output::redirect(log, 2)?;
    Ok(())
}

/// The Graph child a Task run starts.
fn task_run_args(cli: &Cli, issue: &str, owner: &str, graph: &str) -> Vec<String> {
    let mut args = cli.step_args();
    if let Some(wave) = &cli.wave {
        args.extend(["--wave".to_owned(), wave.clone()]);
    }
    args.extend(["--task", issue, owner, "--", graph].map(str::to_owned));
    args
}

/// Drive the Graph in this process, bracketed by graph journal events.
fn execute(
    graph_name: &str,
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
    report_outcome(block_on(async {
        let running = RunningGraph {
            store: open_graph_store().await?,
            process: journal::current_lf_process_id()
                .context("a Graph requires a registered Process")?,
            graph: graph_name,
            steps: items,
            message,
            cwd: repo,
            launcher: cli,
            position: Mutex::new(ExecutionCursor::default()),
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
        // The Graph process's one record of its Graph, written before any step runs.
        running
            .store
            .sqlite
            .record_graph_process(
                &running.process,
                &crate::graph::compiled::CompiledGraph::new(graph_name, items),
            )
            .context("could not record the Graph; no steps launched")?;
        drop(admission);
        let outcome = drive(&running, accounts).await?;
        // A step that completed its Task could not clean up under its own live
        // Graph; the finished Graph can.
        if let (GraphOutcome::Completed, Some(task)) = (&outcome, &running.task) {
            let task = running
                .store
                .get_task(task)
                .await?
                .context("Graph Task disappeared")?;
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

async fn open_graph_store() -> Result<SharedStore> {
    let config = crate::store::storage_config_from_env()?;
    Ok(std::sync::Arc::new(
        crate::store::open_store(&config).await?,
    ))
}

fn report_outcome(outcome: GraphOutcome) -> Result<()> {
    match outcome {
        GraphOutcome::Completed => Ok(()),
        GraphOutcome::Waiting => Err(crate::process::GraphHeld(
            "Graph stopped before its last step; inspect its history and effects".into(),
        )
        .into()),
        GraphOutcome::Blocked(reason) => {
            Err(crate::process::GraphHeld(format!("Graph is blocked: {reason}")).into())
        }
    }
}

/// Run every step from the first until the Graph completes, stops or blocks.
/// A Graph process that dies leaves its Processes as history; nothing resumes it.
async fn drive(
    running: &RunningGraph<'_>,
    accounts: crate::provider_account::selection::AccountSelection,
) -> Result<GraphOutcome> {
    let fields = |extra: LfEventFields| LfEventFields {
        graph: Some(running.graph.to_owned()),
        ..extra
    };
    journal::emit(
        running.cwd,
        LfNode::Graph,
        LfEventType::Started,
        fields(LfEventFields::default()),
    );
    let result = async {
        let _graph_env = EnvVarGuard::set("LOOPFLOW_GRAPH_NAME", running.graph);
        let _accounts = accounts.activate()?;
        let mut cursor = ExecutionCursor::default();
        GraphRunner::new(running)
            .run_with_cursor(running.steps, &mut cursor)
            .await
    }
    .await;
    let (event, error) = match &result {
        Ok(GraphOutcome::Completed) => (LfEventType::Completed, None),
        Ok(_) => (LfEventType::Escalated, None),
        Err(error) => (LfEventType::Errored, Some(error.to_string())),
    };
    journal::emit(
        running.cwd,
        LfNode::Graph,
        event,
        fields(LfEventFields {
            error,
            ..Default::default()
        }),
    );
    result
}

fn print_pipeline_header(graph_name: &str, items: &[ConcreteStep]) {
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
        "\n{dim}\u{2500}\u{2500} graph {reset}{bold}{name}{reset}\n{pipeline}\n",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = graph_name,
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

/// The one Graph executor. It owns the cursor and the Graph's record. Each step
/// is an ordinary command run as a child process; its result is read from the
/// Process that child registered.
struct RunningGraph<'a> {
    store: SharedStore,
    process: crate::id::LfProcessId,
    graph: &'a str,
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
    /// Held or interrupted: the Graph stops here without an automatic retry.
    Stopped,
}

impl RunningGraph<'_> {
    /// The node the runner stands on and the returns taken to reach it.
    fn location(&self) -> (u32, Vec<Vec<u32>>) {
        let cursor = self.position.lock().expect("Graph position mutex poisoned");
        crate::graph::compiled::location(self.steps, &cursor)
            .expect("the runner's position selects a captured node")
    }

    /// The captured step the runner is about to run.
    fn current(&self) -> Option<ConcreteStep> {
        let position = self.position.lock().expect("Graph position mutex poisoned");
        let (body, leaf) = position.current_body(self.steps);
        body.get(leaf.index).cloned()
    }

    /// Record the Process this Graph process's newest child registered, once it has.
    fn record_step(&self, mark: i64) -> Result<Option<crate::id::LfProcessId>> {
        let Some(step) = self.store.sqlite.child_process_after(&self.process, mark)? else {
            return Ok(None);
        };
        let (key, iterations) = self.location();
        self.store
            .sqlite
            .record_graph_step(&self.process, &step, key, &iterations)?;
        Ok(Some(step))
    }

    /// Run `lf <args>` as a child and record its Process as this Graph's next step.
    async fn spawn(
        &self,
        label: &str,
        args: &[String],
    ) -> Result<(StepExit, Option<crate::id::LfProcessId>)> {
        // The absolute selected path becomes argv[0] in the child's Process record.
        let mut command = tokio::process::Command::new(crate::os_process::resolve_lf_binary()?);
        command.current_dir(self.cwd);
        command.env(graph_process::GRAPH_ID_ENV, self.process.as_str());
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
        let mut child = command.spawn().context("could not execute Graph step")?;
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
                    "this Graph process cannot read the result of {label}; inspect its Process and effects with a compatible lf before launching further work"
                )
            })?;
        let status = status.context("could not execute Graph step")?;
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
impl SkillExecutor for &RunningGraph<'_> {
    async fn checkpoint(&self, cursor: &ExecutionCursor) -> Result<()> {
        *self.position.lock().expect("Graph position mutex poisoned") = cursor.clone();
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
        let output = current.as_ref().and_then(GraphOutput::for_step);
        let mut message = self.message.unwrap_or_default().to_owned();
        if skill.returns.is_some() {
            let position = self.position.lock().expect("Graph position mutex poisoned");
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
        if let Some(instructions) = current
            .as_ref()
            .and_then(GraphOutput::for_step_instructions)
        {
            message.push_str(&instructions);
        }
        let mut step_cli = self.launcher.process_options();
        // spawn carries verbosity for skills, operations, and correction turns.
        step_cli.verbose = false;
        step_cli.output_schema = output.as_ref().map(GraphOutput::schema);
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

    /// The operation is its own `lf` command; the Graph process reads how it ended.
    async fn run_command(
        &self,
        ops: &crate::graph::ConcreteCommand,
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
                eprintln!("Landing {landing} is still being watched; the Graph stops here.");
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
    use crate::graph::ConcreteStep;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn linear_on_demand_graph_steps_acquire_before_spawning() {
        use crate::graph::runner::{
            ExecutionContext, ExecutionCursor, SkillExecutor, SkillOutcome,
        };
        use crate::graph::{Command, ConcreteCommand, ConcreteSkill, Skill};
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
                human: false,
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
            let running = super::RunningGraph {
                store: fixture.store.clone(),
                process: crate::id::LfProcessId::new(),
                graph: "probe",
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
        let graphs_dir = temp.path().join(".lf/graphs/tend");
        fs::create_dir_all(&graphs_dir).unwrap();
        fs::write(
            graphs_dir.join("tune.yaml"),
            "- tend/play-chord\n- tend/review-chord\n",
        )
        .unwrap();

        fs::write(
            temp.path().join(".lf/graphs/tend.yaml"),
            "- step: tend/scan-waves\n- xor:\n    router: tend/assess\n    paths:\n      tune:\n        graph: tend/tune\n        description: Adjust the chord\n      silence:\n        description: No-op\n",
        ).unwrap();
        let graph = crate::graph::load_graph("tend", temp.path()).unwrap();

        let items = crate::graph::compile_graph(&graph, temp.path()).unwrap();
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
    fn builtin_task_graphs_launch_as_operational_work() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in ["task-design", "pursue", "queue", "ship"] {
            let graph = crate::graph::load_graph(name, &repo).unwrap();
            let items = crate::graph::compile_graph(&graph, &repo).unwrap();
            super::require_autonomous_steps(&items)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        }
    }

    #[test]
    fn validation_lists_human_nodes_hidden_in_xor_paths() {
        let repo = tempdir().unwrap();
        let graphs = repo.path().join(".lf/graphs");
        fs::create_dir_all(&graphs).unwrap();
        fs::write(
            graphs.join("choice.yaml"),
            "- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - step:\n              id: revise_choice\n              name: implement\n          - step:\n              id: review_choice\n              name: review-design\n              human: true\n",
        )
        .unwrap();
        let graph = crate::graph::load_graph("choice", repo.path()).unwrap();
        let human = crate::graph::human_occurrence_ids(&graph, repo.path()).unwrap();
        assert_eq!(human, vec!["review_choice"]);
        let items = crate::graph::compile_graph(&graph, repo.path()).unwrap();
        assert!(render_pipeline_lines(&items)
            .iter()
            .any(|line| line.contains("review-design [review:review_choice]")));
    }

    #[test]
    fn graph_steps_run_headless_unless_asked_for_a_terminal() {
        use clap::Parser;
        let mode = |argv: &[&str]| {
            let cli = crate::lf::Cli::try_parse_from(argv).unwrap();
            let args = task_run_args(&cli, "INF-1", "graph", "autonomous");
            assert!(
                args.ends_with(&["--task", "INF-1", "graph", "--", "autonomous"].map(String::from))
            );
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
