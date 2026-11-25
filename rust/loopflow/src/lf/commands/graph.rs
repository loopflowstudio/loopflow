use crate::lf::{Cli, Commands, PrCommand};
use anyhow::{Context, Result};
use async_trait::async_trait;
use clap::Parser;
use loopflow_execution::config::environment::EnvVarGuard;
use loopflow_execution::engine::task::TaskRun;
use loopflow_execution::graph::compile_graph;
use loopflow_execution::graph::output::GraphOutput;
use loopflow_execution::graph::return_target;
use loopflow_execution::graph::runner::ExecutionContext;
use loopflow_execution::graph::runner::ExecutionCursor;
use loopflow_execution::graph::runner::GraphOutcome;
use loopflow_execution::graph::runner::GraphRunner;
use loopflow_execution::graph::runner::SkillExecutor;
use loopflow_execution::graph::runner::SkillOutcome;
use loopflow_execution::graph::runner::StepProgress;
use loopflow_execution::graph::ConcreteSkill;
use loopflow_execution::graph::ConcreteStep;
use loopflow_execution::graph::ConcreteXor;
use loopflow_execution::graph::GraphDefinition;
use loopflow_execution::journal::{self, LfEventFields, LfEventType, LfNode};
use loopflow_execution::ops::task::{TaskRunMode, TaskRunStart};
use loopflow_execution::ops::WorkBinding;
use loopflow_execution::ops::{graph_process, tracker_events};
use loopflow_execution::output::Colors;
use loopflow_execution::process::launch_log::{release_launcher, PendingLog};
use loopflow_execution::process::GraphHeld;
use loopflow_execution::store::SharedStore;
use loopflow_model::durable::WorkRef;
use loopflow_model::id::LfProcessId;
use loopflow_storage::graph_process::RunOwner;
use loopflow_storage::sqlite::{AutoState, SqliteStore};
use loopflow_storage::work::task::Task;
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
    // A named Graph runs here with the checkout's context; only `task run`
    // advances the Task's Graph and requires admission to start Task work.
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
        None,
    )
}

/// `lf --task ISSUE graph run`: carry the Task's Graph from the cursor its
/// `lf task run`, this process's parent, recorded: where the run began, then
/// the step after each one finished. The Graph writes where it left the Task:
/// before the interactive step it stopped at, or at `end`.
pub fn run_task_graph(cli: &Cli, repo: &Path, binding: Option<&WorkBinding>) -> Result<()> {
    let Some(WorkRef::Task(task)) = binding.map(|binding| &binding.work) else {
        anyhow::bail!("`lf graph run` carries a Task's Graph; `lf task run` starts it");
    };
    let store = block_on(open_graph_store())?;
    store.sqlite.require_task_launch(task)?;
    let row = loopflow_execution::store::task_work::task_graph_row(&store.sqlite, task)?;
    let Some((graph, (start, carrier))) = row.and_then(|row| row.run.map(|run| (row.graph, run)))
    else {
        anyhow::bail!("the Task is not on a run; `lf task run` starts one");
    };
    let parent = journal::current_parent_lf_process_id();
    anyhow::ensure!(
        parent.as_ref() == Some(&carrier.id),
        "the Task's run is carried by another `lf task run`; read `lf task status`"
    );
    print_pipeline_header(&graph.name, &graph.steps);
    execute(
        &graph.name,
        &graph.steps,
        start,
        None,
        cli,
        repo,
        binding,
        Some(&carrier.id),
    )
}

/// What `lf graph customize` wrote.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Customized {
    /// The repository file that defines the Graph.
    pub path: std::path::PathBuf,
}

/// `lf graph customize <name>` in `directory`'s repository.
pub fn customize(name: &str, directory: &Path) -> Result<Customized> {
    let repo = loopflow_execution::repo::working_directory_in(directory)?;
    Ok(Customized {
        path: loopflow_execution::graph::customize(name, &repo)?,
    })
}

pub fn show(name: &str, repo: &Path) -> Result<()> {
    let graph = loopflow_execution::graph::load_authored_graph(name, repo)?;
    let items = compile_graph(&graph, repo)?;
    for line in render_pipeline_lines(&items) {
        println!("{line}");
    }
    Ok(())
}

/// `lf graph list [--json]` — Graph definitions with the topology each would capture.
pub fn list(repo: &Path, json: bool) -> Result<()> {
    let catalog = loopflow_execution::graph::compiled::graph_catalog(repo)?;
    if json {
        println!("{}", serde_json::to_string(&catalog)?);
        return Ok(());
    }
    for line in catalog_lines(&catalog) {
        println!("{line}");
    }
    Ok(())
}

/// One line per Graph: its name, then its description or why it can't load.
fn catalog_lines(
    catalog: &[loopflow_execution::graph::compiled::GraphCatalogEntry],
) -> Vec<String> {
    let width = catalog
        .iter()
        .map(|entry| entry.name.len())
        .max()
        .unwrap_or(0);
    catalog
        .iter()
        .map(|entry| {
            let detail = match (&entry.unavailable, &entry.description) {
                (Some(reason), _) => format!("(unavailable: {reason})"),
                (None, Some(description)) => description.clone(),
                (None, None) => String::new(),
            };
            if detail.is_empty() {
                entry.name.clone()
            } else {
                format!("{:width$}  {detail}", entry.name)
            }
        })
        .collect()
}

/// How many times one `lf task run` starts its Graph before giving up.
const TASK_RUN_ATTEMPTS: u32 = 3;

/// A Task run that must stop rather than retry, auto on or not.
#[derive(Debug, thiserror::Error)]
enum TaskRunStopped {
    #[error("the run of {run} failed {TASK_RUN_ATTEMPTS} times with the same error; this Task run stops here: {cause}")]
    Repeated { run: String, cause: String },
    #[error(
        "the run of {run} stopped before any agent started, and `lf task run {issue}` starts it again there: {cause}"
    )]
    Unlaunched {
        issue: String,
        run: String,
        cause: String,
    },
    #[error(
        "the run of {run} stopped after a provider refusal at {step}: {cause}; `lf task run {issue}` starts the run again at it"
    )]
    Limit {
        issue: String,
        run: String,
        step: String,
        cause: String,
    },
    #[error(
        "the run of {run} stopped at {step}: {cause}; {}",
        loopflow_execution::ops::task_execution::question_advice(.issue)
    )]
    Question {
        issue: String,
        run: String,
        step: String,
        cause: String,
    },
}

impl TaskRunStopped {
    /// Auto stays as it was: the next `lf task run` stands in again.
    fn stop(self, status: std::process::ExitStatus) -> Result<()> {
        // Render before the detached run's log closes when its process exits.
        eprintln!("{self}");
        Err(anyhow::Error::new(exit_of(status)).context(self))
    }
}

/// Carry one Task run: start the Task's Graph as a child Graph process, and
/// retry failed Graphs unless a provider refusal stopped them, a skill step
/// failed before its agent started, or its agent asked a question nobody was
/// there to answer. Each attempt records
/// its own GraphExecution beneath this process and sets out at the step the last
/// one failed at. A Graph held for a person or a
/// watcher, an interrupted one, and a launch refused before any Graph started
/// are returned as they ended. Once someone interrupts the Task, nothing is
/// started again.
/// `run` names the graph and where the run set out, for messages.
///
/// A bare run inside its execution owner lets its launcher go once the first
/// attempt has registered, and carries on with its output in that attempt's
/// log. `announce` hears the first registered Graph process and that log,
/// before the launcher goes.
pub fn run_task_graph_for(
    cli: &Cli,
    task: &Task,
    run: &str,
    cursor: i64,
    announce: &mut dyn FnMut(&LfProcessId, Option<&Path>) -> Result<()>,
) -> Result<()> {
    let checkout = task.worktree()?;
    let args = task_run_args(cli, task.id.as_str(), &["graph", "run"]);
    let store = block_on(open_graph_store())?;
    let store = &store.sqlite;
    let process =
        journal::current_lf_process_id().context("a Task run requires a registered Process")?;
    let letting_go = lets_launcher_go(cli);
    let mut released = false;
    let mut attempt = 1;
    // The error every failed attempt so far shared, if they all shared one.
    let mut repeated: Option<String> = None;
    loop {
        // This process mints the attempt's Graph run id; the child runs as it.
        let graph_run = LfProcessId::default();
        let mut log = letting_go
            .then(|| PendingLog::open(&format!("pending-{process}-{attempt}")))
            .transpose()?;
        let sink = log.as_ref().map(|log| log.file.try_clone()).transpose()?;
        let status = spawn_task_child(&args, checkout, sink, Some(cursor), &graph_run, |child| {
            let registered = registration(store, &graph_run, child)?;
            match (registered, log.take()) {
                (true, Some(mut log)) => {
                    log.settle(&graph_run)?;
                    if !released {
                        announce(&graph_run, Some(&log.path))?;
                        release_launcher(&log.file)?;
                        released = true;
                    }
                }
                (true, None) if !released => {
                    announce(&graph_run, None)?;
                    released = true;
                }
                (false, Some(log)) => log.replay(),
                _ => {}
            }
            Ok(())
        })?;
        // A child that died before writing its end leaves what this process saw.
        record_spawner_end(store, RunOwner::GraphRun, &graph_run, status.code(), None);
        if status.success() {
            return Ok(());
        }
        if store.task_graph_stopped_since(&task.id, cursor)? {
            eprintln!("Task run interrupted; not starting it again.");
            return Err(loopflow_execution::process::CommandExit(130).into());
        }
        // Only a Graph process that failed, or that a signal nobody sends to
        // stop it killed, is started again.
        let failed = (status.code() == Some(1)
            || (status.code().is_none() && !interrupted(status)))
            && store.graph_run_recorded(&graph_run)?;
        if failed {
            if let Some(stash) = loopflow_execution::ops::checkout::retained_edits(checkout)? {
                eprintln!(
                    "The run of {run} failed with the checkout's edits retained in stash {stash}; this Task run stops here. Restore them with `git stash apply --index {stash}`."
                );
                return Err(loopflow_execution::process::CommandExit(1).into());
            }
            let cause = store
                .graph_failure_cause(&graph_run)?
                .map(|(_, cause)| cause);
            // A launch lf refused would be refused again: the next attempt
            // has nothing to retry.
            if store.graph_failed_before_its_agent(&graph_run)? {
                let stop = TaskRunStopped::Unlaunched {
                    issue: task.identifier.clone(),
                    run: run.to_string(),
                    cause: cause.unwrap_or_else(|| "it failed".into()),
                };
                return stop.stop(status);
            }
            // A question nobody answered would only be asked again.
            if let Some(question) = store.graph_unanswered_question(&graph_run)? {
                let stop = TaskRunStopped::Question {
                    issue: task.identifier.clone(),
                    run: run.to_string(),
                    step: stopped_step(store, task)?.unwrap_or_else(|| "its step".into()),
                    cause: question,
                };
                return stop.stop(status);
            }
            repeated = cause.filter(|cause| attempt == 1 || repeated.as_ref() == Some(cause));
            if let Some((step, recovery)) =
                loopflow_execution::ops::task::limit_recovery::ended_step_recovery(store, &task.id)?
            {
                // The recovery sweep restarts a wait at its reset, not now.
                if let Some(retry_at) = recovery
                    .retry_at
                    .filter(|at| *at > loopflow_storage::rows::now_unix())
                {
                    eprintln!(
                        "The run of {run} is waiting on a limit until {}; it starts again at {step} then.",
                        loopflow_execution::output::format_time(retry_at),
                    );
                    return Err(exit_of(status).into());
                }
                if let Some(cause) = loopflow_execution::ops::task_execution::limit_stop(&recovery)
                {
                    let stop = TaskRunStopped::Limit {
                        issue: task.identifier.clone(),
                        run: run.to_string(),
                        step,
                        cause,
                    };
                    return stop.stop(status);
                }
            }
        }
        if !failed || attempt == TASK_RUN_ATTEMPTS {
            if failed {
                if let Some(cause) = repeated {
                    let repeated = TaskRunStopped::Repeated {
                        run: run.to_string(),
                        cause,
                    };
                    return repeated.stop(status);
                }
                eprintln!("The run of {run} failed {attempt} times; this Task run stops here.");
            }
            return Err(exit_of(status).into());
        }
        let again =
            stopped_step(store, task)?.map_or_else(String::new, |step| format!(" at {step}"));
        eprintln!(
            "The run of {run} failed (attempt {attempt} of {TASK_RUN_ATTEMPTS}); starting it again{again}."
        );
        attempt += 1;
    }
}

/// The step the Task's Graph stopped at: a failed Graph checkpointed after
/// each step it finished.
fn stopped_step(store: &SqliteStore, task: &Task) -> Result<Option<String>> {
    Ok(
        loopflow_execution::store::task_work::task_graph_row(store, &task.id)?
            .and_then(|row| row.graph.step_name_of(&row.run?.0)),
    )
}

/// `lf task run`: place the Task and carry its run in this process. What the
/// run set out to do is announced once, as `task.run` returns it: printed,
/// and reported to the launcher that started this process detached.
pub fn carry_task_run(
    cli: &Cli,
    repo: &Path,
    issue: &str,
    options: loopflow_execution::ops::task::TaskProcessOptions,
    json: bool,
) -> Result<()> {
    let (task, start) = loopflow_execution::ops::task::task_place(
        repo,
        issue,
        options,
        task_run_mode(cli),
        crate::lf::navigation::command_merges,
    )?;
    carry_placed_task(cli, task, start, json)
}

pub fn carry_unpaused_task(cli: &Cli, repo: &Path, issue: &str) -> Result<()> {
    let placed = loopflow_execution::ops::task::unpause_task_run(
        repo,
        issue,
        task_run_mode(cli),
        crate::lf::navigation::command_merges,
    )?;
    let Some((task, start)) = placed else {
        loopflow_execution::engine::launch::report(Ok(&None::<TaskRun>));
        return Ok(());
    };
    carry_placed_task(cli, task, start, true)
}

fn task_run_mode(cli: &Cli) -> TaskRunMode {
    if cli.interactive {
        TaskRunMode::InPlace
    } else if cli.batch {
        TaskRunMode::Headless
    } else {
        TaskRunMode::Stop
    }
}

fn carry_placed_task(cli: &Cli, task: Task, start: TaskRunStart, json: bool) -> Result<()> {
    if !cli.interactive {
        loopflow_execution::ops::task::prepare_conversation_beside(&task);
    }
    let issue = task.identifier.clone();
    let announce = |run: &TaskRun| -> Result<()> {
        loopflow_execution::engine::launch::report(Ok(run));
        if json {
            println!("{}", serde_json::to_string(run)?);
        } else if let Some(text) = run.render() {
            println!("{text}");
        }
        Ok(())
    };
    let (step, merges, cursor) = match start {
        TaskRunStart::Run {
            step,
            merges,
            stands_in,
            interrupt_cursor,
        } => {
            if let Some(step) = stands_in {
                eprintln!("{issue}: standing in at {step}");
            }
            (step, merges, interrupt_cursor)
        }
        TaskRunStart::Live { run, graph_process } => {
            return announce(&TaskRun::Running {
                log: Some(loopflow_execution::process::launch_log::graph_log_path(
                    graph_process.as_str(),
                )),
                issue,
                run,
                merges: true,
                graph_process,
            })
        }
        TaskRunStart::Waiting(step) => return announce(&TaskRun::Waiting { issue, step }),
        TaskRunStart::Ended => return announce(&TaskRun::Ended { issue }),
    };
    // This process carries the run; its Graph process writes where the
    // Task stopped. A run that never arrives leaves the Task to start it again.
    run_task_graph_for(cli, &task, &step, cursor, &mut |graph, log| {
        announce(&TaskRun::Running {
            issue: issue.clone(),
            run: step.clone(),
            merges,
            graph_process: graph.clone(),
            log: log.map(Path::to_path_buf),
        })
    })?;
    Ok(loopflow_execution::ops::task::task_run_finished(&task)?)
}

/// Whether a Graph process ended on a signal people and lf send to stop one:
/// ctrl-C, `lf task interrupt`, or its terminal closing. Any other signal,
/// such as a SIGKILL, killed it.
fn interrupted(status: std::process::ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    matches!(
        status.signal(),
        Some(libc::SIGINT | libc::SIGTERM | libc::SIGHUP)
    )
}

fn exit_of(status: std::process::ExitStatus) -> loopflow_execution::process::CommandExit {
    let code = match status.code().and_then(|code| u8::try_from(code).ok()) {
        Some(code) => code,
        None if interrupted(status) => 130,
        None => 1,
    };
    loopflow_execution::process::CommandExit(code)
}

/// What a spawner saw of a run that may have died before writing its end.
/// Best-effort: a busy store must not fail a run whose child succeeded or
/// replace the error that stopped it; the child's own write and the liveness
/// read cover a missed one.
fn record_spawner_end(
    store: &SqliteStore,
    owner: RunOwner,
    run: &LfProcessId,
    exit_code: Option<i32>,
    error: Option<&str>,
) {
    if let Err(store_error) = store.finish_run(
        owner,
        run,
        loopflow_storage::rows::now_unix(),
        exit_code,
        error,
    ) {
        tracing::warn!(error = %store_error, %run, "could not record the end of a run this process spawned");
    }
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
    graph_run: &LfProcessId,
    started: impl FnOnce(&mut std::process::Child) -> Result<()>,
) -> Result<std::process::ExitStatus> {
    static ATTEMPT_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    loopflow_execution::agent::register_interrupt_cleanup(|| {
        let pid = ATTEMPT_PID.load(std::sync::atomic::Ordering::Acquire);
        if pid != 0 {
            loopflow_execution::platform::kill_process(pid);
        }
    });
    let lf = loopflow_execution::os_process::resolve_lf_binary()?;
    let mut command = std::process::Command::new(&lf);
    command.args(args).current_dir(checkout);
    command.env(
        loopflow_execution::journal::LF_EXECUTION_ID_ENV,
        graph_run.as_str(),
    );
    if let Some(cursor) = interrupt {
        command.env(graph_process::TASK_INTERRUPT_CURSOR_ENV, cursor.to_string());
    }
    loopflow_execution::transport::configure_child(&mut command);
    let output = match log {
        Some(log) => {
            command
                .stdin(std::process::Stdio::null())
                .stdout(log.try_clone()?)
                .stderr(log);
            None
        }
        None => loopflow_execution::terminal_output::TerminalOutput::for_child(&mut command)?,
    };
    let mut child = command.spawn().context("could not start the Task's step")?;
    ATTEMPT_PID.store(child.id(), std::sync::atomic::Ordering::Release);
    started(&mut child)?;
    let status = child.wait();
    drop(output);
    ATTEMPT_PID.store(0, std::sync::atomic::Ordering::Release);
    status.context("could not wait for the Task's step")
}

/// Whether the attempt recorded its Graph run before it ended, so a Task run
/// that lets its launcher go can settle its output. No deadline: the child's
/// own exit bounds the wait.
fn registration(
    store: &SqliteStore,
    graph_run: &LfProcessId,
    child: &mut std::process::Child,
) -> Result<bool> {
    loop {
        if store.graph_run_recorded(graph_run)? {
            return Ok(true);
        }
        if child.try_wait()?.is_some() {
            return Ok(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Whether this run lets whoever started it go once it is under way: it runs
/// in a held terminal or detached, and is neither `-b` nor interactive.
pub(crate) fn lets_launcher_go(cli: &Cli) -> bool {
    !cli.batch
        && !cli.interactive
        && (loopflow_execution::transport::current().is_some()
            || loopflow_execution::os_process::detached::launched())
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
    task_run: Option<&LfProcessId>,
) -> Result<()> {
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
            task_run: task_run.cloned(),
            interrupt,
        };
        // A project rotation moving this checkout's Task excludes new work in it.
        let admission = loopflow_execution::store::admission::lock_task_checkouts(
            &running.store.sqlite,
            &[running.cwd],
            running.task.as_ref(),
        )?;
        // The Graph process's one record of its Graph, written before any step runs.
        running
            .store
            .sqlite
            .record_graph(
                &running.process,
                &loopflow_execution::graph::compiled::compile(graph_name, items),
                running.task.as_ref(),
                &journal::current_execution_start()
                    .context("a Graph requires a registered Process")?,
                journal::current_repo().as_deref(),
                Some(&running.cwd.display().to_string()),
                loopflow_execution::transport::held_in().as_ref(),
            )
            .context("could not record the Graph; no steps launched")?;
        journal::mark_run_owner(RunOwner::GraphRun);
        drop(admission);
        let outcome = drive(&running, start).await?;
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
                GraphOutcome::Completed => Some(loopflow_execution::task_graph::END),
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
            loopflow_execution::ops::task::retirement::schedule(&running.store, &task).await?;
        }
        Ok(outcome)
    })?)
}

/// The interrupt cursor of the Task run this Graph belongs to, or its own when
/// it was started directly.
async fn interrupt_cursor(
    store: &SharedStore,
    task: &loopflow_model::durable::TaskId,
) -> Result<i64> {
    if let Some(cursor) = std::env::var(graph_process::TASK_INTERRUPT_CURSOR_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
    {
        return Ok(cursor);
    }
    Ok(store
        .latest_task_event(task)
        .await?
        .map_or(0, |event| event.id))
}

/// Run `future` to completion from this synchronous command.
fn block_on<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(future)
}

async fn open_graph_store() -> Result<SharedStore> {
    let config = loopflow_execution::store::storage_config_from_env()?;
    Ok(std::sync::Arc::new(
        loopflow_execution::store::open_store(&config).await?,
    ))
}

fn report_outcome(outcome: GraphOutcome) -> Result<()> {
    match outcome {
        // Each stop printed why as it stopped.
        GraphOutcome::Completed | GraphOutcome::StoppedAt(_) => Ok(()),
        GraphOutcome::Interrupted => Err(loopflow_execution::process::CommandExit(130).into()),
        GraphOutcome::Waiting => Err(GraphHeld(
            "Graph stopped before its last step; inspect its history and effects".into(),
        )
        .into()),
        GraphOutcome::Blocked(reason) => {
            Err(GraphHeld(format!("{} {reason}", GraphHeld::BLOCKED_PREFIX)).into())
        }
    }
}

/// Run every step from the first until the Graph completes, stops or blocks.
/// A Graph process that dies leaves its Processes as history; nothing resumes it.
async fn drive(running: &RunningGraph<'_>, start: ExecutionCursor) -> Result<GraphOutcome> {
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
        let mut cursor = start;
        GraphRunner::new(running)
            .run_with_cursor(running.steps, &mut cursor)
            .await
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

/// The one Graph executor. It owns the cursor and the Graph's record. Each step
/// is an ordinary command run as a child process; its result is read from the
/// Process that child registered.
struct RunningGraph<'a> {
    store: SharedStore,
    process: loopflow_model::id::LfProcessId,
    graph: &'a str,
    steps: &'a [ConcreteStep],
    message: Option<&'a str>,
    cwd: &'a Path,
    launcher: &'a Cli,
    /// Where the runner stands, as of its last checkpoint.
    position: Mutex<ExecutionCursor>,
    task: Option<loopflow_model::durable::TaskId>,
    /// The `lf task run` this Graph carries its Task for: a stop keeps it from
    /// merging, and each checkpoint is where that run's next attempt sets out.
    task_run: Option<LfProcessId>,
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

/// What a child of the Graph process is: whether an interrupted Task may end
/// it early, and whether it is a step at all.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StepKind {
    /// An agent turn ends at once; its Session records the interruption.
    Agent,
    /// An operation always finishes: stopping one midway strands its work.
    Operation,
    /// The commit after a skill step always finishes too, and is no step:
    /// the Graph's steps stay the ones it declares.
    Commit,
}

impl RunningGraph<'_> {
    /// The node the runner stands on and the returns taken to reach it.
    fn location(&self) -> (u32, Vec<Vec<u32>>) {
        let cursor = self.position.lock().expect("Graph position mutex poisoned");
        loopflow_execution::graph::compiled::location(self.steps, &cursor)
            .expect("the runner's position selects a captured node")
    }

    /// The captured step the runner is about to run.
    fn current(&self) -> Option<&ConcreteStep> {
        let position = self.position.lock().expect("Graph position mutex poisoned");
        let (body, leaf) = position.current_body(self.steps);
        body.get(leaf.index)
    }

    /// Reserve the step about to spawn under the id minted for it, so the
    /// step finds its row however fast it ends.
    fn reserve_step(&self, step: &LfProcessId) -> Result<()> {
        let (key, iterations) = self.location();
        let start = loopflow_storage::graph_process::ExecutionStart::reserved(
            loopflow_storage::rows::now_unix(),
        );
        self.store
            .sqlite
            .record_graph_step(&self.process, step, key, &iterations, &start)?;
        Ok(())
    }

    /// Where a Task run waits instead of merging: the step about to run merges
    /// and someone turned auto off. Read when the step is reached, so a stop
    /// made mid-run counts; an unreadable mark fails the step.
    fn stopped_landing(&self) -> Result<Option<String>> {
        let Some(task) = self.task.as_ref().filter(|_| self.task_run.is_some()) else {
            return Ok(None);
        };
        let Some(step) = self.current().and_then(|step| {
            loopflow_execution::task_graph::landing_name(
                step,
                crate::lf::navigation::command_merges,
            )
        }) else {
            return Ok(None);
        };
        let Some(stop) = self
            .store
            .sqlite
            .task_auto(task)?
            .filter(|mark| mark.state == AutoState::Off)
        else {
            return Ok(None);
        };
        let issue = self
            .store
            .sqlite
            .task(task)?
            .map(|task| task.identifier)
            .ok_or_else(|| anyhow::anyhow!("Task {task} is not recorded"))?;
        eprintln!(
            "{}",
            loopflow_execution::ops::task::auto::landing_wait(&step, &stop, &issue)
        );
        Ok(Some(step))
    }

    /// Whether this run stands in for the person at interactive `skill`, or
    /// stops before it (`None`). `-b` always stands in and `-i` runs it with
    /// the person here; a bare Task run reads auto as it gets there, so
    /// turning it on mid-run counts. A Graph outside a Task run launches the
    /// step as `lf skill` would.
    fn stands_in(&self, skill: &ConcreteSkill) -> Result<Option<bool>> {
        if !skill.interactive || self.launcher.interactive {
            return Ok(Some(false));
        }
        if self.launcher.batch {
            return Ok(Some(true));
        }
        let Some(task) = self.task.as_ref().filter(|_| self.task_run.is_some()) else {
            return Ok(Some(false));
        };
        let on = self
            .store
            .sqlite
            .task_auto(task)?
            .is_some_and(|mark| mark.state == AutoState::On);
        Ok(on.then_some(true))
    }

    /// Whether someone interrupted this Graph's Task after its run began.
    fn interrupted(&self) -> bool {
        let (Some(task), Some(cursor)) = (&self.task, self.interrupt) else {
            return false;
        };
        match self.store.sqlite.task_graph_stopped_since(task, cursor) {
            Ok(stopped) => stopped,
            Err(error) => {
                tracing::warn!(%error, "failed to read Task interrupts");
                false
            }
        }
    }

    /// Run `lf <args>` as a child and record its Process as this Graph's next
    /// step, unless it is the commit that follows one.
    async fn spawn(
        &self,
        label: &str,
        kind: StepKind,
        args: &[String],
    ) -> Result<(StepExit, Option<loopflow_model::id::LfProcessId>)> {
        if self.interrupted() {
            eprintln!("Graph {} interrupted before {label}.", self.graph);
            return Ok((StepExit::Interrupted, None));
        }
        // The absolute selected path becomes argv[0] in the child's Process record.
        let mut command =
            tokio::process::Command::new(loopflow_execution::os_process::resolve_lf_binary()?);
        command.current_dir(self.cwd);
        command.env(graph_process::GRAPH_ID_ENV, self.process.as_str());
        // The step runs as the id this Graph mints and records for it.
        let run = LfProcessId::default();
        command.env(
            loopflow_execution::journal::LF_EXECUTION_ID_ENV,
            run.as_str(),
        );
        if kind == StepKind::Commit {
            loopflow_execution::config::settings::independent_agent(command.as_std_mut())?;
        }
        let step = (kind != StepKind::Commit).then_some(run);
        command.env(
            loopflow_execution::journal::GRAPH_STEP_ENV,
            if self.task_run.is_some() {
                "worker"
            } else {
                "bare"
            },
        );
        command.env_remove(graph_process::TASK_INTERRUPT_CURSOR_ENV);
        if self.launcher.verbose {
            command.arg("--verbose");
        }
        command.args(args);
        if let Some(step) = &step {
            self.reserve_step(step)?;
        }
        // Once reserved, the step ends with whatever stopped it from starting.
        let spawned = (|| -> Result<_> {
            let admission = loopflow_execution::store::admission::lock_task_checkouts(
                &self.store.sqlite,
                &[self.cwd],
                self.task.as_ref(),
            )?;
            loopflow_execution::transport::configure_child(command.as_std_mut());
            let output = loopflow_execution::terminal_output::TerminalOutput::for_child(
                command.as_std_mut(),
            )?;
            let child = command.spawn().context("could not execute Graph step")?;
            Ok((admission, output, child))
        })();
        let (admission, output, mut child) = match spawned {
            Ok(spawned) => spawned,
            Err(error) => {
                if let Some(step) = &step {
                    record_spawner_end(
                        &self.store.sqlite,
                        RunOwner::GraphStep,
                        step,
                        None,
                        Some(&format!("{error:#}")),
                    );
                }
                return Err(error);
            }
        };
        drop(admission);
        if let (Some(step), Some(pid)) = (&step, child.id()) {
            let birth = loopflow_execution::process::os_process::process_started_at(pid)
                .ok()
                .flatten();
            self.store.sqlite.record_graph_step_pid(step, pid, birth)?;
        }
        let mut watch = tokio::time::interval(std::time::Duration::from_secs(1));
        watch.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let directions = tracker_events::watch_owned_work(self.task.clone());
        tokio::pin!(directions);
        let mut signalled = false;
        let status = loop {
            tokio::select! {
                status = child.wait() => break status,
                _ = &mut directions, if kind == StepKind::Agent && self.task.is_some() => {},
                _ = watch.tick(), if kind == StepKind::Agent && !signalled => {
                    if self.interrupted() {
                        if let Some(pid) = child.id() {
                            loopflow_execution::platform::kill_process(pid);
                        }
                        signalled = true;
                    }
                }
            }
        };
        drop(output);
        // A step that died before writing its end leaves what this Graph saw.
        if let Some(step) = &step {
            let (code, error) = match &status {
                Ok(status) => (status.code(), None),
                Err(error) => (
                    None,
                    Some(format!("could not wait for Graph step: {error}")),
                ),
            };
            record_spawner_end(
                &self.store.sqlite,
                RunOwner::GraphStep,
                step,
                code,
                error.as_deref(),
            );
        }
        // A new installation can select a newer child. Never read its result
        // through an older schema.
        self.store
            .sqlite
            .validate_current_schema(&loopflow_execution::store::migrations::build())
            .with_context(|| {
                format!(
                    "this Graph run cannot read the result of {label}; inspect the step and its effects with a compatible lf before launching further work"
                )
            })?;
        let status = status.context("could not execute Graph step")?;
        // An interrupted agent ends at once; a draining operation finishes and
        // records its own exit so the runner advances past it before stopping.
        if signalled || (kind == StepKind::Agent && self.interrupted()) {
            eprintln!("Graph {} interrupted at {label}.", self.graph);
            // Someone stopped it: a wait it was in no longer starts it again.
            if let Some(step) = &step {
                self.store.sqlite.end_agent_limit_wait(step)?;
            }
            return Ok((StepExit::Interrupted, step));
        }
        match status.code() {
            Some(0) => Ok((StepExit::Finished, step)),
            // A turn someone else interrupted, such as a native takeover, is held.
            Some(3 | 130) => Ok((StepExit::Held, step)),
            _ => {
                // The step's input says how its last turn ended.
                if let Some((_, input)) = self.captured(step.as_ref())? {
                    if let Some(end) = self.store.sqlite.input_turn_end(&input)? {
                        if end.status.as_deref() == Some("interrupted") {
                            return Ok((StepExit::Held, step));
                        }
                        if let Some(error) = end.error {
                            anyhow::bail!("{label}: {error}");
                        }
                    }
                }
                anyhow::bail!("{label} process exited with {status}")
            }
        }
    }

    /// The conversation and input a step's Process captured; none for an operation.
    fn captured(
        &self,
        step: Option<&loopflow_model::id::LfProcessId>,
    ) -> Result<Option<(String, String)>> {
        let Some(step) = step else { return Ok(None) };
        Ok(self
            .store
            .sqlite
            .process_input(step)?
            .map(|(session, input, _)| (session, input)))
    }

    /// The Session a step's Process opened and the answer its turn returned.
    fn answer(
        &self,
        label: &str,
        step: Option<&loopflow_model::id::LfProcessId>,
    ) -> Result<(String, Option<String>)> {
        let step = step.with_context(|| format!("{label} registered no step"))?;
        let (session, input) = self
            .captured(Some(step))?
            .with_context(|| format!("{label} step {} captured no Session input", step.as_str()))?;
        let answer =
            loopflow_execution::transcript::answer::input_final_answer(&self.store.sqlite, &input)?;
        Ok((session, answer))
    }
}

/// What a headless agent reads at an interactive step. With a loop behind the
/// step it answers advance or iterate; blocked repeats with the blocker.
fn stand_in_message(step: &str, repeats: bool) -> String {
    let person = loopflow_execution::config::participant_name()
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

impl RunningGraph<'_> {
    /// Every skill step's edits become one commit, written and pushed by a
    /// child `lf commit --step` (which also pushes commits the skill made
    /// itself), so its message-writing agent runs in a Process of its own.
    /// The commit is no step: it adds none to the Graph's record. A Graph
    /// contributing to a Task that another run carries leaves the checkout's
    /// edits to that run. Best-effort: the next step runs either way.
    async fn commit_step(&self, step: &str) {
        let contribution = self.task.is_some() && self.task_run.is_none();
        if contribution
            || !matches!(
                loopflow_execution::git::intervention_state(self.cwd),
                Ok(None)
            )
            || !loopflow_execution::ops::step_commit_pending(self.cwd)
        {
            return;
        }
        let args = ["commit", "--step", step].map(str::to_owned);
        if let Err(error) = self.spawn("commit", StepKind::Commit, &args).await {
            tracing::warn!(%step, %error, "could not commit the step's edits");
        }
    }
}

#[async_trait]
impl SkillExecutor for &RunningGraph<'_> {
    async fn checkpoint(&self, cursor: &ExecutionCursor) -> Result<()> {
        *self.position.lock().expect("Graph position mutex poisoned") = cursor.clone();
        // A Task run that fails starts again here, not where it began. A stale
        // cursor reruns finished steps; that beats failing a finished step.
        if let (Some(task), Some(carrier)) = (&self.task, &self.task_run) {
            if let Err(error) = loopflow_execution::store::task_work::advance_task_graph_run(
                &self.store.sqlite,
                task,
                carrier,
                cursor,
            ) {
                tracing::warn!(%task, %error, "could not record where the Task run goes on");
            }
        }
        Ok(())
    }

    async fn run_skill(
        &self,
        skill: &ConcreteSkill,
        ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        if self.interrupted() {
            eprintln!(
                "Graph {} interrupted before {}.",
                self.graph, skill.skill.name
            );
            return Ok(SkillOutcome::Interrupted);
        }
        let Some(standing_in) = self.stands_in(skill)? else {
            let step = skill.interactive_step().unwrap_or(&skill.skill.name);
            eprintln!("Stopped at {step}; the Task waits there for its conversation.");
            return Ok(SkillOutcome::StopsAt(step.to_owned()));
        };
        if let Some(step) = self.stopped_landing()? {
            return Ok(SkillOutcome::StopsAt(step));
        }
        loopflow_execution::ops::tracker_events::sync_repository(
            &self.store,
            &self.cwd.to_string_lossy(),
        )
        .await;
        let name = &skill.skill.name;
        print_step_progress(ctx.progress, name);
        let mut message = self.message.unwrap_or_default().to_owned();
        let step = {
            let position = self.position.lock().expect("Graph position mutex poisoned");
            let (body, leaf) = position.current_body(self.steps);
            if let Some(start) = return_target(body, leaf.index) {
                let traversals = leaf.progress.repeats.get(&leaf.index.to_string());
                let pass = u64::from(traversals.copied().unwrap_or(0)) + 1;
                let steps = leaf.index - start + 1;
                let target = loopflow_execution::task_graph::step_name(&body[start]);
                message.push_str(&format!("\n\nDecision pass {pass}. Iterating repeats {steps} steps, back to {target}.\nCompare what the preceding pass set out to do with what changed or was learned. Say when earlier results are missing."));
            }
            body.get(leaf.index)
        };
        if let Some(direction) = &ctx.direction {
            message.push_str(&format!(
                "\n\nPrevious step feedback or iteration direction:\n{direction}"
            ));
        }
        // Standing in, an interactive step runs headless for the person.
        if standing_in {
            let step = skill.interactive_step().unwrap_or(name);
            message.push_str(&stand_in_message(step, skill.returns.is_some()));
        }
        let output =
            step.and_then(|step| GraphOutput::append_step_instructions(step, &mut message));
        let mut step_cli = self.launcher.process_options();
        // spawn carries verbosity for skills, operations, and correction turns.
        step_cli.verbose = false;
        step_cli.output_schema = output.as_ref().map(GraphOutput::schema);
        // `-i` and `-b` choose for every step. Otherwise a step runs headless,
        // and an interactive step no one stands in at launches the way
        // `lf skill` would: with the person at a terminal, else headless.
        step_cli.interactive = self.launcher.interactive;
        step_cli.batch = !self.launcher.interactive && (!skill.interactive || standing_in);
        // The child receives this occurrence's captured source, not a later
        // catalog selection. Keep the file alive through every child/readback.
        let mut input = tempfile::NamedTempFile::new()?;
        serde_json::to_writer(
            &mut input,
            &loopflow_execution::skills::invocation::SkillInvocation {
                skill: skill.skill.clone(),
                arguments: self.message.unwrap_or_default().to_string(),
            },
        )?;
        step_cli.skill_input = Some(input.path().to_path_buf());
        let mut args = step_cli.step_args();
        // A Task run's step is the Task's own conversation, not a bare `lf`
        // in its checkout, so it names the Task.
        if let Some(task) = self.task.as_ref().filter(|_| self.task_run.is_some()) {
            args.extend(["--task".to_owned(), task.to_string()]);
        }
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
            self.commit_step(name).await;
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
                "steer".to_owned(),
                conversation,
                format!("The previous final output was invalid: {error}. Return a corrected value using the declared schema; preserve the preceding work.{}", output.instructions()),
            ];
        }
        unreachable!("the correction loop returns or fails")
    }

    /// The operation is its own `lf` command; the Graph process reads how it ended.
    async fn run_command(
        &self,
        ops: &loopflow_execution::graph::ConcreteCommand,
        _ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        let label = ops.item.display_name();
        if self.interrupted() {
            eprintln!("Graph {} interrupted before {label}.", self.graph);
            return Ok(SkillOutcome::Interrupted);
        }
        if let Some(step) = self.stopped_landing()? {
            return Ok(SkillOutcome::StopsAt(step));
        }
        loopflow_execution::ops::tracker_events::sync_repository(
            &self.store,
            &self.cwd.to_string_lossy(),
        )
        .await;
        eprintln!("op: {label}");
        let argv = ops.item.argv();
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
            }) => {
                Some(loopflow_execution::ops::land::resolve_repos(self.cwd, worktree.as_deref())?.0)
            }
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
    loopflow_execution::transport::report(loopflow_model::program_status::State::Clear, true, None);
    loopflow_execution::transport::report(
        loopflow_model::program_status::State::Working,
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
    use loopflow_execution::graph::ConcreteStep;
    use std::fs;
    use tempfile::tempdir;

    /// Record the Graph run `execute` writes before any step, which each
    /// step's row references.
    fn record_run(running: &super::RunningGraph<'_>) {
        running
            .store
            .sqlite
            .record_graph(
                &running.process,
                &loopflow_execution::graph::compiled::compile(running.graph, running.steps),
                running.task.as_ref(),
                &loopflow_storage::graph_process::ExecutionStart::reserved(0),
                None,
                None,
                None,
            )
            .unwrap();
    }

    #[test]
    fn linear_on_demand_graph_steps_acquire_before_spawning() {
        use crate::lf::Cli;
        use clap::Parser;
        use loopflow_execution::graph::runner::{
            ExecutionContext, ExecutionCursor, SkillExecutor, SkillOutcome,
        };
        use loopflow_execution::graph::{Command, ConcreteCommand, ConcreteSkill, Skill};
        use loopflow_execution::test_support::planning_tasks::{
            mark_issue_updated, with_planning_task,
        };
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
                interactive_name: None,
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
                process: loopflow_model::id::LfProcessId::new(),
                graph: "probe",
                steps: &steps,
                message: None,
                cwd: repo,
                launcher: &cli,
                position: Mutex::new(ExecutionCursor::default()),
                task: None,
                task_run: None,
                interrupt: None,
            };
            record_run(&running);
            runtime.block_on(async {
                let conn = rusqlite::Connection::open(&fixture.database).unwrap();
                for index in 0..2 {
                    conn.execute("DELETE FROM planning_scope_checks", [])
                        .unwrap();
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
        use crate::lf::Cli;
        use clap::Parser;
        use loopflow_execution::graph::runner::{ExecutionCursor, GraphOutcome, GraphRunner};
        use loopflow_execution::graph::{ConcreteSkill, Skill};
        use loopflow_execution::test_support::planning_tasks::with_planning_task;
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
                interactive_name: None,
                returns: None,
                sources: Vec::new(),
            });
            let steps = [skill.clone(), skill];
            let work = loopflow_model::durable::WorkRef::Task(task.id.clone());
            let running = super::RunningGraph {
                store: fixture.store.clone(),
                process: loopflow_model::id::LfProcessId::new(),
                graph: "probe",
                steps: &steps,
                message: None,
                cwd: repo,
                launcher: &cli,
                position: Mutex::new(ExecutionCursor::default()),
                task: Some(task.id.clone()),
                task_run: None,
                interrupt: Some(
                    runtime
                        .block_on(super::interrupt_cursor(&fixture.store, &task.id))
                        .unwrap(),
                ),
            };
            record_run(&running);
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
    fn a_draining_command_preserves_failure_and_advances_when_it_exits_zero() {
        use crate::lf::Cli;
        use clap::Parser;
        use loopflow_execution::graph::runner::{ExecutionCursor, GraphOutcome, GraphRunner};
        use loopflow_execution::graph::{Command, ConcreteCommand};
        use loopflow_execution::test_support::planning_tasks::with_planning_task;
        use std::os::unix::fs::PermissionsExt;
        use std::sync::Mutex;

        with_planning_task(|runtime, fixture, repo, task, _| {
            let _env = crate::test_ambient::EnvGuard::clear(&["LF_BIN"]);
            let work = loopflow_model::durable::WorkRef::Task(task.id.clone());
            let cli = Cli::try_parse_from(["lf", "--batch"]).unwrap();
            let op = |name: &str| {
                ConcreteStep::Command(ConcreteCommand {
                    item: Command {
                        command: name.into(),
                        args: Vec::new(),
                    },
                    sources: Vec::new(),
                })
            };

            // A draining command that exits non-zero fails instead of reading Interrupted.
            let started = fixture.directory.path().join("started-fail");
            let release = fixture.directory.path().join("release-fail");
            let child = fixture.directory.path().join("lf-fail");
            fs::write(
                &child,
                format!(
                    "#!/bin/sh\n: > '{}'\nwhile [ ! -f '{}' ]; do sleep 0.02; done\nexit 1\n",
                    started.display(),
                    release.display()
                ),
            )
            .unwrap();
            fs::set_permissions(&child, fs::Permissions::from_mode(0o755)).unwrap();
            std::env::set_var("LF_BIN", &child);
            let steps = [op("first"), op("second")];
            let running = super::RunningGraph {
                store: fixture.store.clone(),
                process: loopflow_model::id::LfProcessId::new(),
                graph: "drain-fail",
                steps: &steps,
                message: None,
                cwd: repo,
                launcher: &cli,
                position: Mutex::new(ExecutionCursor::default()),
                task: Some(task.id.clone()),
                task_run: None,
                interrupt: Some(
                    runtime
                        .block_on(super::interrupt_cursor(&fixture.store, &task.id))
                        .unwrap(),
                ),
            };
            record_run(&running);
            let result = runtime.block_on(async {
                let interrupt = async {
                    while !started.exists() {
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    }
                    fixture.store.append_interrupt(&work).await.unwrap();
                    fs::write(&release, "").unwrap();
                };
                let runner = GraphRunner::new(&running);
                let (result, ()) = tokio::join!(runner.run(&steps, 0), interrupt);
                result
            });
            assert!(
                result.is_err(),
                "a non-zero draining command must fail, not read Interrupted: {result:?}"
            );

            // A draining final command that exits 0 completes the Graph.
            let started = fixture.directory.path().join("started-last");
            let release = fixture.directory.path().join("release-last");
            let child = fixture.directory.path().join("lf-ok");
            fs::write(
                &child,
                format!(
                    "#!/bin/sh\n: > '{}'\nwhile [ ! -f '{}' ]; do sleep 0.02; done\nexit 0\n",
                    started.display(),
                    release.display()
                ),
            )
            .unwrap();
            fs::set_permissions(&child, fs::Permissions::from_mode(0o755)).unwrap();
            std::env::set_var("LF_BIN", &child);
            let last_step = [op("final")];
            let running = super::RunningGraph {
                store: fixture.store.clone(),
                process: loopflow_model::id::LfProcessId::new(),
                graph: "drain-last",
                steps: &last_step,
                message: None,
                cwd: repo,
                launcher: &cli,
                position: Mutex::new(ExecutionCursor::default()),
                task: Some(task.id.clone()),
                task_run: None,
                interrupt: Some(
                    runtime
                        .block_on(super::interrupt_cursor(&fixture.store, &task.id))
                        .unwrap(),
                ),
            };
            record_run(&running);
            let outcome = runtime.block_on(async {
                let interrupt = async {
                    while !started.exists() {
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    }
                    fixture.store.append_interrupt(&work).await.unwrap();
                    fs::write(&release, "").unwrap();
                };
                let runner = GraphRunner::new(&running);
                let (outcome, ()) = tokio::join!(runner.run(&last_step, 0), interrupt);
                outcome.unwrap()
            });
            assert_eq!(outcome, GraphOutcome::Completed);
            assert_eq!(running.position.lock().unwrap().index, 1);
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
        let graph = loopflow_execution::graph::load_graph("tend", temp.path()).unwrap();

        let items = loopflow_execution::graph::compile_graph(&graph, temp.path()).unwrap();
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
        for name in ["task-design", "pursue", "queue", "deliver"] {
            let graph = loopflow_execution::graph::load_graph(name, &repo).unwrap();
            let items = loopflow_execution::graph::compile_graph(&graph, &repo).unwrap();
            assert!(
                items
                    .iter()
                    .all(|item| !matches!(item, ConcreteStep::Skill(skill) if skill.interactive)),
                "{name} has an interactive step"
            );
        }
        for name in ["feature", "code", "research"] {
            let graph = loopflow_execution::task_graph::CapturedGraph::load(name, &repo)
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
        let graph = loopflow_execution::graph::load_graph("choice", repo.path()).unwrap();
        let items = loopflow_execution::graph::compile_graph(&graph, repo.path()).unwrap();
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
