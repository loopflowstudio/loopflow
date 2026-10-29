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
        ExecutionCursor::default(),
        bound_message.as_deref().or(message),
        cli,
        repo,
        binding,
        false,
        None,
    )
}

/// `lf --task ISSUE graph run`: carry the Task's Graph from the cursor its
/// `lf task run`, this process's parent, set out from. The Graph writes where
/// it left the Task: before the interactive step it stopped at, or at `end`.
pub fn run_task_graph(cli: &Cli, repo: &Path, binding: Option<&WorkBinding>) -> Result<()> {
    let Some(WorkRef::Task(task)) = binding.map(|binding| &binding.work) else {
        anyhow::bail!("`lf graph run` carries a Task's Graph; `lf task run` starts it");
    };
    let store = block_on(open_graph_store())?;
    store.sqlite.require_task_launch(task)?;
    let row = store.sqlite.task_graph_row(task)?;
    let Some((graph, (start, carrier))) = row.and_then(|row| row.run.map(|run| (row.graph, run)))
    else {
        anyhow::bail!("the Task is not on a run; `lf task run` starts one");
    };
    let parent = journal::current_lf_process_id()
        .and_then(|id| store.sqlite.process(&id).ok().flatten())
        .and_then(|process| process.parent_lf_process_id);
    anyhow::ensure!(
        parent.as_ref() == Some(&carrier.id),
        "the Task's run is carried by another `lf task run`; read `lf task status`"
    );
    print_pipeline_header(&graph.name, &graph.steps);
    // `-b` and `-i` run straight through: every step headless, or every step
    // with the person in this terminal.
    let stop_at_interactive = !cli.batch && !cli.interactive;
    execute(
        &graph.name,
        &graph.steps,
        start,
        None,
        cli,
        repo,
        binding,
        stop_at_interactive,
        Some(&carrier.id),
    )
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

/// Carry one Task run: start the Task's Graph as a child Graph process, and
/// start it again while a Graph process fails. Each attempt records its own
/// GraphProcess beneath this process. A Graph held for a person or a watcher, an
/// interrupted one, and a launch refused before any Graph started are returned
/// as they ended. Once someone interrupts the Task, nothing is started again.
/// `run` names the graph and where the run set out, for messages.
///
/// With `release`, a bare run inside its execution owner lets its launcher go
/// once the first attempt has registered, and carries on with its output in
/// that attempt's log.
pub fn run_task_graph_for(cli: &Cli, task: &Task, run: &str, release: bool) -> Result<()> {
    let checkout = task.worktree()?;
    let args = task_run_args(cli, task.id.as_str(), &["graph", "run"]);
    let store = block_on(open_graph_store())?;
    let store = &store.sqlite;
    let process =
        journal::current_lf_process_id().context("a Task run requires a registered Process")?;
    let work = WorkRef::Task(task.id.clone());
    let cursor = store.latest_interrupt_id(&work)?;
    let letting_go =
        release && !cli.batch && !cli.interactive && crate::transport::current().is_some();
    let mut released = false;
    let mut attempt = 1;
    loop {
        let mark = store.process_mark(&process)?;
        let mut log = letting_go
            .then(|| PendingLog::open(&format!("pending-{process}-{attempt}")))
            .transpose()?;
        let sink = log.as_ref().map(|log| log.file.try_clone()).transpose()?;
        let status = spawn_task_child(&args, checkout, sink, Some(cursor), |child| {
            let Some(mut log) = log.take() else {
                return Ok(());
            };
            match registration(store, &process, mark, child)? {
                Some(registered) => {
                    log.settle(&registered)?;
                    if !released {
                        println!(
                            "{} · {run} running as Graph process {registered}\n  follow: lf task status {}\n  log:    {}",
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
            Ok(())
        })?;
        if status.success() {
            return Ok(());
        }
        if store.latest_interrupt_id(&work)? > cursor {
            eprintln!("Task run interrupted; not starting it again.");
            return Err(crate::process::CommandExit(130).into());
        }
        // Only a Graph process that failed is started again.
        let failed = status.code() == Some(1) && attempt_graph(store, &process, mark)?.is_some();
        if failed {
            if let Some(stash) = crate::ops::checkout::retained_edits(checkout)? {
                eprintln!(
                    "The run of {run} failed with the checkout's edits retained in stash {stash}; this Task run stops here. Restore them with `git stash apply --index {stash}`."
                );
                return Err(crate::process::CommandExit(1).into());
            }
        }
        if !failed || attempt == TASK_RUN_ATTEMPTS {
            if failed {
                eprintln!("The run of {run} failed {attempt} times; this Task run stops here.");
            }
            return Err(exit_of(status).into());
        }
        eprintln!(
            "The run of {run} failed (attempt {attempt} of {TASK_RUN_ATTEMPTS}); starting it again."
        );
        attempt += 1;
    }
}

fn exit_of(status: std::process::ExitStatus) -> crate::process::CommandExit {
    let code = status.code().and_then(|code| u8::try_from(code).ok());
    crate::process::CommandExit(code.unwrap_or(130))
}

/// Start `lf <args>` in `checkout` and wait for it, writing to `log` when given
/// and to this terminal otherwise. A Graph child receives the Task's interrupt
/// cursor; a skill's agent shell must never inherit it. `started` runs once the
/// child is up. An interrupted caller takes the running child with it.
fn spawn_task_child(
    args: &[String],
    checkout: &Path,
    log: Option<std::fs::File>,
    interrupt: Option<i64>,
    started: impl FnOnce(&mut std::process::Child) -> Result<()>,
) -> Result<std::process::ExitStatus> {
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
    if let Some(cursor) = interrupt {
        command.env(graph_process::TASK_INTERRUPT_CURSOR_ENV, cursor.to_string());
    }
    crate::transport::configure_child(&mut command);
    let output = match log {
        Some(log) => {
            command
                .stdin(std::process::Stdio::null())
                .stdout(log.try_clone()?)
                .stderr(log);
            None
        }
        None => crate::terminal_output::TerminalOutput::for_child(&mut command)?,
    };
    let mut child = command.spawn().context("could not start the Task's step")?;
    ATTEMPT_PID.store(child.id(), std::sync::atomic::Ordering::Release);
    started(&mut child)?;
    let status = child.wait();
    drop(output);
    ATTEMPT_PID.store(0, std::sync::atomic::Ordering::Release);
    status.context("could not wait for the Task's step")
}

/// Where a Task run that let its launcher go keeps its Graph process's output.
pub fn graph_log_path(id: &str) -> std::path::PathBuf {
    crate::store::lf_home_dir()
        .join("logs")
        .join(format!("graph-{id}.log"))
}

/// A run's output once its launcher lets go: an attempt's until it registers,
/// when it becomes that Graph process's log, or a ship's for its whole run.
#[derive(Debug)]
pub(crate) struct PendingLog {
    pub(crate) file: std::fs::File,
    pub(crate) path: std::path::PathBuf,
}

impl PendingLog {
    pub(crate) fn open(name: &str) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let path = graph_log_path(name);
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

/// Wait until the attempt records its GraphProcess or ends first. No deadline:
/// the child's own exit bounds it.
fn registration(
    store: &SqliteStore,
    process: &LfProcessId,
    mark: i64,
    child: &mut std::process::Child,
) -> Result<Option<LfProcessId>> {
    loop {
        let registered = attempt_graph(store, process, mark)?;
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
pub(crate) fn release_launcher(log: &std::fs::File) -> Result<()> {
    use std::io::Write;
    std::io::stdout().flush()?;
    std::io::stderr().flush()?;
    crate::transport::detach_viewers()?;
    crate::terminal_output::redirect(log, 1)?;
    crate::terminal_output::redirect(log, 2)?;
    Ok(())
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

/// Drive the Graph in this process from `start`, bracketed by graph journal events.
#[allow(clippy::too_many_arguments)]
fn execute(
    graph_name: &str,
    items: &[ConcreteStep],
    start: ExecutionCursor,
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
    binding: Option<&WorkBinding>,
    stop_at_interactive: bool,
    task_run: Option<&LfProcessId>,
) -> Result<()> {
    let accounts = crate::provider_account::selection::AccountSelection::from_flags_or_env(
        &cli.account,
        &cli.only_account,
    )?;
    report_outcome(block_on(async {
        let store = open_graph_store().await?;
        let task = binding.and_then(|binding| match &binding.work {
            WorkRef::Task(id) => Some(id.clone()),
            _ => None,
        });
        let interrupt = match &task {
            Some(task) => Some(interrupt_cursor(&store, task).await?),
            None => None,
        };
        let running = RunningGraph {
            store,
            process: journal::current_lf_process_id()
                .context("a Graph requires a registered Process")?,
            graph: graph_name,
            steps: items,
            message,
            cwd: repo,
            launcher: cli,
            position: Mutex::new(start.clone()),
            task,
            interrupt,
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
        let outcome = drive(&running, start, accounts, stop_at_interactive).await?;
        // A Task run's Graph writes where it left the Task. Blocked at an
        // interactive step, the Task waits there: what blocks it is for a
        // person, and nothing before it runs again.
        if let (Some(carrier), Some(task)) = (task_run, &running.task) {
            let blocked_at = match (&outcome, running.current()) {
                (GraphOutcome::Blocked(_), Some(ConcreteStep::Skill(skill))) => {
                    skill.interactive_step().map(str::to_owned)
                }
                _ => None,
            };
            let step = match &outcome {
                GraphOutcome::Completed => Some(crate::task_graph::END),
                GraphOutcome::StoppedAt(step) => Some(step.as_str()),
                _ => blocked_at.as_deref(),
            };
            if let Some(step) = step {
                running
                    .store
                    .sqlite
                    .arrive_task_graph(task, carrier, step)?;
            }
        }
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

/// The interrupt cursor of the Task run this Graph belongs to, or its own when
/// it was started directly.
async fn interrupt_cursor(store: &SharedStore, task: &crate::durable::TaskId) -> Result<i64> {
    if let Some(cursor) = std::env::var(graph_process::TASK_INTERRUPT_CURSOR_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
    {
        return Ok(cursor);
    }
    Ok(store
        .latest_interrupt_id(&WorkRef::Task(task.clone()))
        .await?)
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
        GraphOutcome::StoppedAt(step) => {
            eprintln!(
                "Stopped at {step}, an interactive step; the Task conversation takes it from here."
            );
            Ok(())
        }
        GraphOutcome::Interrupted => Err(crate::process::CommandExit(130).into()),
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
    start: ExecutionCursor,
    accounts: crate::provider_account::selection::AccountSelection,
    stop_at_interactive: bool,
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
        let mut cursor = start;
        let runner = GraphRunner::new(running);
        let runner = if stop_at_interactive {
            runner.stopping_at_interactive()
        } else {
            runner
        };
        runner.run_with_cursor(running.steps, &mut cursor).await
    }
    .await;
    let (event, error) = match &result {
        Ok(GraphOutcome::Completed | GraphOutcome::StoppedAt(_)) => (LfEventType::Completed, None),
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
            ConcreteStep::Skill(skill) => match skill.interactive_step() {
                Some(step) if step != skill.skill.name => {
                    lines.push(format!("{} [interactive {step}]", skill.skill.name))
                }
                Some(_) => lines.push(format!("{} [interactive]", skill.skill.name)),
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
    /// The Task's latest interrupt when its run began; a later one stops the Graph.
    interrupt: Option<i64>,
}

/// How a step's process ended, before any answer is read.
enum StepExit {
    Finished,
    /// Held for a person or a watcher: the Graph stops here without a retry.
    Held,
    /// Someone interrupted the Task: the Graph starts nothing further.
    Interrupted,
}

/// Whether the Graph process may end a step early when its Task is interrupted.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StepKind {
    /// An agent turn ends at once; its Session records the interruption.
    Agent,
    /// An operation always finishes: stopping one midway strands its work.
    Operation,
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

    /// Whether someone interrupted this Graph's Task after its run began.
    fn interrupted(&self) -> bool {
        let (Some(task), Some(cursor)) = (&self.task, self.interrupt) else {
            return false;
        };
        match self
            .store
            .sqlite
            .latest_interrupt_id(&WorkRef::Task(task.clone()))
        {
            Ok(latest) => latest > cursor,
            Err(error) => {
                tracing::warn!(%error, "failed to read Task interrupts");
                false
            }
        }
    }

    /// Run `lf <args>` as a child and record its Process as this Graph's next step.
    async fn spawn(
        &self,
        label: &str,
        kind: StepKind,
        args: &[String],
    ) -> Result<(StepExit, Option<crate::id::LfProcessId>)> {
        if self.interrupted() {
            eprintln!("Graph {} interrupted before {label}.", self.graph);
            return Ok((StepExit::Interrupted, None));
        }
        // The absolute selected path becomes argv[0] in the child's Process record.
        let mut command = tokio::process::Command::new(crate::os_process::resolve_lf_binary()?);
        command.current_dir(self.cwd);
        command.env(graph_process::GRAPH_ID_ENV, self.process.as_str());
        command.env_remove(graph_process::TASK_INTERRUPT_CURSOR_ENV);
        if self.launcher.verbose {
            command.arg("--verbose");
        }
        command.args(args);
        let mark = self.store.sqlite.process_mark(&self.process)?;
        let admission = self
            .store
            .sqlite
            .lock_task_checkouts(&[self.cwd], self.task.as_ref())?;
        crate::transport::configure_child(command.as_std_mut());
        let output = crate::terminal_output::TerminalOutput::for_child(command.as_std_mut())?;
        let mut child = command.spawn().context("could not execute Graph step")?;
        drop(admission);
        let mut step = None;
        let mut watch = tokio::time::interval(std::time::Duration::from_secs(1));
        watch.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut signalled = false;
        let status = loop {
            tokio::select! {
                status = child.wait() => break status,
                // A read can fail while a newer child migrates; the exit retries it.
                () = tokio::time::sleep(std::time::Duration::from_millis(10)), if step.is_none() => {
                    step = self.record_step(mark).unwrap_or(None);
                }
                _ = watch.tick(), if kind == StepKind::Agent && !signalled => {
                    if self.interrupted() {
                        if let Some(pid) = child.id() {
                            crate::platform::kill_process(pid);
                        }
                        signalled = true;
                    }
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
        // An interrupt during an operation takes effect once it has finished.
        if signalled || self.interrupted() {
            eprintln!("Graph {} interrupted at {label}.", self.graph);
            return Ok((StepExit::Interrupted, step));
        }
        match status.code() {
            Some(0) => Ok((StepExit::Finished, step)),
            // A turn someone else interrupted, such as a native takeover, is held.
            Some(3 | 130) => Ok((StepExit::Held, step)),
            _ => {
                // The step's own conversation says how its turn ended.
                for envelope in self.turn_events(step.as_ref())?.iter().rev() {
                    let event = &envelope["event"];
                    if event["type"] == "turn_completed" && event["status"] == "interrupted" {
                        return Ok((StepExit::Held, step));
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

/// What a headless agent reads at an interactive step. With a loop behind the
/// step it answers advance or iterate; blocked repeats with the blocker.
fn stand_in_message(step: &str, repeats: bool) -> String {
    let person = crate::config::participant_name()
        .ok()
        .flatten()
        .unwrap_or_else(|| "the person who started this run".into());
    let mut message = format!(
        "\n\nNo one is available. You are standing in for {person} at {step}. Decide as they would, record why in your final message, and continue."
    );
    if repeats {
        message.push_str(" Advance when the work before this step holds; iterate only for a concrete, repairable gap, naming it; there is no one to hand a blocker to.");
    }
    message
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
        // `-b` runs an interactive step headless: the agent stands in for the person.
        if skill.interactive && !self.launcher.interactive && self.launcher.batch {
            let step = skill.interactive_step().unwrap_or(name);
            message.push_str(&stand_in_message(step, skill.returns.is_some()));
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
        // `-i` and `-b` choose for every step. Otherwise a step runs headless,
        // and an interactive step launches the way `lf skill` would: with the
        // person at a terminal, else headless.
        step_cli.interactive = self.launcher.interactive;
        step_cli.batch = !self.launcher.interactive && (self.launcher.batch || !skill.interactive);
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
            let (exit, step) = self.spawn(name, StepKind::Agent, &args).await?;
            match exit {
                StepExit::Finished => {}
                StepExit::Held => return Ok(SkillOutcome::Waiting),
                StepExit::Interrupted => return Ok(SkillOutcome::Interrupted),
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
        let argv = ops.item.argv();
        let parsed = crate::lf::navigation::normalize_args(argv.clone())
            .and_then(Cli::try_parse_from)
            .ok();
        let handoff_checkout = match parsed.and_then(|cli| cli.command) {
            Some(Commands::Ops {
                cmd:
                    crate::lf::OpsCommand::Pr {
                        cmd:
                            Some(PrCommand::Land {
                                local: false,
                                wait_and_fix: false,
                                worktree,
                                ..
                            }),
                    },
            }) => Some(crate::ops::land::resolve_repos(self.cwd, worktree.as_deref())?.0),
            _ => None,
        };
        let args: Vec<String> = argv.into_iter().skip(1).collect();
        match self.spawn(&label, StepKind::Operation, &args).await?.0 {
            StepExit::Finished => {}
            StepExit::Held => return Ok(SkillOutcome::Waiting),
            StepExit::Interrupted => return Ok(SkillOutcome::Interrupted),
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
                interrupt: None,
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
                    // One delta, then the Project its changed issue names.
                    assert_eq!(state.lock().await.requests, 2);
                }
            });
        });
    }

    #[test]
    fn an_interrupt_ends_the_running_agent_step_and_starts_nothing_further() {
        use crate::graph::runner::{ExecutionCursor, GraphOutcome, GraphRunner};
        use crate::graph::{ConcreteSkill, Skill};
        use crate::lf::Cli;
        use crate::ops::linear::task_planning_tests::with_planning_task;
        use clap::Parser;
        use std::os::unix::fs::PermissionsExt;
        use std::sync::Mutex;

        with_planning_task(|runtime, fixture, repo, task, _| {
            let _env = crate::test_ambient::EnvGuard::clear(&["LF_BIN"]);
            let started = fixture.directory.path().join("started");
            let child = fixture.directory.path().join("lf-step");
            fs::write(
                &child,
                format!(
                    "#!/bin/sh\necho >> '{}'\nexec sleep 30\n",
                    started.display()
                ),
            )
            .unwrap();
            fs::set_permissions(&child, fs::Permissions::from_mode(0o755)).unwrap();
            std::env::set_var("LF_BIN", &child);
            let cli = Cli::try_parse_from(["lf", "--batch"]).unwrap();
            let skill = ConcreteStep::Skill(ConcreteSkill {
                skill: Skill::named("probe"),
                id: None,
                interactive: false,
                returns: None,
                sources: Vec::new(),
            });
            let steps = [skill.clone(), skill];
            let work = crate::durable::WorkRef::Task(task.id.clone());
            let running = super::RunningGraph {
                store: fixture.store.clone(),
                process: crate::id::LfProcessId::new(),
                graph: "probe",
                steps: &steps,
                message: None,
                cwd: repo,
                launcher: &cli,
                position: Mutex::new(ExecutionCursor::default()),
                task: Some(task.id.clone()),
                interrupt: Some(
                    runtime
                        .block_on(fixture.store.latest_interrupt_id(&work))
                        .unwrap(),
                ),
            };
            let begun = std::time::Instant::now();
            let outcome = runtime.block_on(async {
                let interrupt = async {
                    while !started.exists() {
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    }
                    fixture.store.append_interrupt(&work).await.unwrap();
                };
                let runner = GraphRunner::new(&running);
                let (outcome, ()) = tokio::join!(runner.run(&steps, 0), interrupt);
                outcome.unwrap()
            });
            assert_eq!(outcome, GraphOutcome::Interrupted);
            assert!(begun.elapsed() < std::time::Duration::from_secs(10));
            assert_eq!(fs::read_to_string(&started).unwrap().lines().count(), 1);
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
    fn builtin_task_graphs_compile_and_name_their_interactive_steps() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in ["task-design", "pursue", "queue", "deliver", "auto"] {
            let graph = crate::graph::load_graph(name, &repo).unwrap();
            let items = crate::graph::compile_graph(&graph, &repo).unwrap();
            assert!(
                items
                    .iter()
                    .all(|item| !matches!(item, ConcreteStep::Skill(skill) if skill.interactive)),
                "{name} has an interactive step"
            );
        }
        for name in ["feature", "code", "research"] {
            let graph = crate::task_graph::CapturedGraph::load(name, &repo)
                .unwrap()
                .unwrap_or_else(|| panic!("{name} is a builtin graph"));
            assert!(
                !graph.interactive_steps().is_empty(),
                "{name} has interactive steps"
            );
        }
    }

    #[test]
    fn pipeline_lines_mark_interactive_steps_hidden_in_xor_paths() {
        let repo = tempdir().unwrap();
        let graphs = repo.path().join(".lf/graphs");
        fs::create_dir_all(&graphs).unwrap();
        fs::write(
            graphs.join("choice.yaml"),
            "- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - step:\n              id: revise_choice\n              name: implement\n          - interactive:\n              id: review_choice\n              name: explore\n",
        )
        .unwrap();
        let graph = crate::graph::load_graph("choice", repo.path()).unwrap();
        let items = crate::graph::compile_graph(&graph, repo.path()).unwrap();
        assert!(render_pipeline_lines(&items)
            .iter()
            .any(|line| line.contains("explore [interactive review_choice]")));
    }

    #[test]
    fn task_run_children_carry_only_the_launch_override() {
        use clap::Parser;
        let mode = |argv: &[&str]| {
            let cli = crate::lf::Cli::try_parse_from(argv).unwrap();
            let args = task_run_args(&cli, "INF-1", &["graph", "run"]);
            assert!(args.ends_with(&["--task", "INF-1", "graph", "run"].map(String::from)));
            (
                args.contains(&"--batch".to_owned()),
                args.contains(&"--interactive".to_owned()),
            )
        };
        // A bare run stops at interactive steps; `-b` runs straight through
        // headless, `-i` with the person at every step.
        assert_eq!(mode(&["lf", "task", "run", "INF-1"]), (false, false));
        assert_eq!(mode(&["lf", "-b", "task", "run", "INF-1"]), (true, false));
        assert_eq!(mode(&["lf", "-i", "task", "run", "INF-1"]), (false, true));
    }
}
