pub mod request;

use std::cell::RefCell;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::{debug, warn};

use crate::git::worktrees::main_repo_root;
use crate::process::{AgentCaller, AGENT_CALLER_ENV};
use loopflow_model::id::{LfProcessId, LfSessionId, TraceId};
use loopflow_storage::graph_process::RunOwner;
use loopflow_storage::sqlite::SqliteStore;

const JOURNAL_ROOT: &str = ".lf/journal/traces";
const JOURNAL_EXCLUDE_ENTRY: &str = ".lf/journal/";
pub const LF_TRACE_ID_ENV: &str = "LF_TRACE_ID";
/// The tracing target of a Start's timings: the server's creation steps and the
/// conversation's setup steps. `logs/lf.log` keeps it at INFO for the report.
pub const START_TIMING_TARGET: &str = "loopflow::start_timing";
pub const LF_PROCESS_ID_ENV: &str = "LF_PROCESS_ID";
/// The Session whose agent started the work this process belongs to,
/// inherited through the lf processes between.
pub const LF_CALLER_SESSION_ENV: &str = "LF_CALLER_SESSION";
/// The id a spawner minted for this process and already recorded on the run
/// it owns (a Graph run or step). Taken once at start; children never see it.
pub const LF_EXECUTION_ID_ENV: &str = "LF_EXECUTION_ID";

/// Serializes tests that mutate process-global store or Process identity variables.
/// Every test in the crate that touches these variables must hold this lock.
#[cfg(any(test, feature = "test-support"))]
pub fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    loopflow_test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static TEST_LEDGER_DB_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug)]
pub struct TestLedgerGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous_lf_home: Option<std::ffi::OsString>,
    previous_test_path: Option<PathBuf>,
    home: tempfile::TempDir,
}

#[cfg(any(test, feature = "test-support"))]
impl Default for TestLedgerGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl TestLedgerGuard {
    pub fn new() -> Self {
        let lock = test_env_lock();
        let home = tempfile::TempDir::new().expect("test ledger home");
        let previous_lf_home = std::env::var_os("LF_HOME");
        std::env::remove_var("LF_HOME");
        std::env::set_var("LF_HOME", home.path());
        let previous_test_path =
            TEST_LEDGER_DB_PATH.with(|path| path.replace(Some(home.path().join("loopflow.db"))));
        Self {
            _lock: lock,
            previous_lf_home,
            previous_test_path,
            home,
        }
    }

    pub fn home(&self) -> &Path {
        self.home.path()
    }

    pub fn set_db_path(&self, path: PathBuf) {
        TEST_LEDGER_DB_PATH.with(|current| *current.borrow_mut() = Some(path));
    }
}

/// The store this test thread is pinned to, for a worker it starts.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn test_ledger_path() -> Option<PathBuf> {
    TEST_LEDGER_DB_PATH.with(|path| path.borrow().clone())
}

/// Pin a blocking fixture worker to its owning test's store; the caller holds
/// the environment lock. Production processes already share one admitted ledger.
#[cfg(any(test, feature = "test-support"))]
pub fn with_test_ledger<T>(path: PathBuf, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<PathBuf>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_LEDGER_DB_PATH.with(|path| *path.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(TEST_LEDGER_DB_PATH.with(|current| current.replace(Some(path))));
    run()
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for TestLedgerGuard {
    fn drop(&mut self) {
        TEST_LEDGER_DB_PATH.with(|path| *path.borrow_mut() = self.previous_test_path.take());
        match &self.previous_lf_home {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }
    }
}

thread_local! {
    static THREAD_CONTEXT: RefCell<Option<ProcessContext>> = const { RefCell::new(None) };
}

// Only the executable entry point sets this. Library calls retain their own
// outer with_runtime scope; inherited environment cannot opt into or out of it.
static PROCESS_STARTED_AT: OnceLock<i64> = OnceLock::new();
static PROCESS_START: OnceLock<Instant> = OnceLock::new();
static EFFECT_FREE_READ: AtomicBool = AtomicBool::new(false);

/// A command that touches no store never becomes a Process: neither admission
/// nor exit observation records it. The executable selects this before any
/// command admission; environment cannot.
pub fn mark_effect_free_read() {
    EFFECT_FREE_READ.store(true, Ordering::Relaxed);
}

static READ: AtomicBool = AtomicBool::new(false);

/// Set by a Graph on the step it starts, and only on that child.
pub const GRAPH_STEP_ENV: &str = "LF_GRAPH_STEP";
static GRAPH_STEP: AtomicBool = AtomicBool::new(false);
static WORKER_STEP: AtomicBool = AtomicBool::new(false);
/// Whether a Graph started this process as its step. Clears the mark, so the
/// step's own children are not steps.
pub fn take_graph_step() -> bool {
    let value = std::env::var_os(GRAPH_STEP_ENV);
    let step = value.is_some();
    WORKER_STEP.fetch_or(
        value.is_some_and(|value| value == "worker"),
        Ordering::Relaxed,
    );
    std::env::remove_var(GRAPH_STEP_ENV);
    GRAPH_STEP.fetch_or(step, Ordering::Relaxed) || step
}

/// Whether a Graph started this process as its step, once taken.
pub(crate) fn graph_step() -> bool {
    GRAPH_STEP.load(Ordering::Relaxed)
}

/// Whether Task delivery started this Graph step as a headless worker.
pub(crate) fn worker_step() -> bool {
    WORKER_STEP.load(Ordering::Relaxed)
}

/// A command that changes no Work writes no journal trace. Admission still
/// prepares the store it reads, so a read works on a fresh Machine.
pub fn mark_read() {
    READ.store(true, Ordering::Relaxed);
}

/// The harness issued this command: a provider hook inherits the agent's caller
/// and the launching lf's Process, but neither the model nor that lf ran it.
/// Call before admission.
pub fn harness_issued() {
    std::env::remove_var(AGENT_CALLER_ENV);
    std::env::remove_var(LF_PROCESS_ID_ENV);
}
// An actual lf process keeps one identity across async and blocking workers.
// Library callers retain the thread-scoped with_runtime lifetime above.
static PROCESS_CONTEXT: Mutex<Option<ProcessContext>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProcessContext {
    trace_id: TraceId,
    lf_process_id: LfProcessId,
    parent_lf_process_id: Option<LfProcessId>,
    agent_caller: Option<AgentCaller>,
    /// The Session whose agent started this work: the caller's, else the
    /// one this process inherited.
    caller_session: Option<LfSessionId>,
    /// Time this command entered the runtime, independent of OS inspection.
    started_at: i64,
    process_started_at: Option<i64>,
    cwd: PathBuf,
    ledger_path: PathBuf,
    /// Retain the first successful connection through Process completion.
    #[serde(skip)]
    ledger: Arc<OnceLock<SqliteStore>>,
    /// Serialized argv captured at Process start so terminal rows name their work.
    command: Option<String>,
    /// Optional repository trace journal. Early process observation uses only
    /// SQLite; ordinary dispatch also writes this when Git exclusion succeeds.
    trace_dir: Option<PathBuf>,
    repo: Option<String>,
    wave: Option<String>,
    #[serde(skip)]
    finished: Arc<AtomicBool>,
    /// True when this process minted the trace id (vs inheriting LF_TRACE_ID);
    /// the export is removed again when the Process ends.
    minted_trace_id: bool,
    /// The records this process runs as and writes its end on: the step its
    /// Graph minted it for, and any Graph run, Task run or Project
    /// activation it recorded.
    #[serde(skip)]
    run_owners: Arc<Mutex<Vec<RunOwner>>>,
}

/// Time since this lf process entered its entry point; `None` in a library caller.
pub(crate) fn process_elapsed() -> Option<Duration> {
    PROCESS_START.get().map(Instant::elapsed)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LfNode {
    #[serde(alias = "exec")] // Historical trace events remain readable.
    Process,
    Graph,
    Skill,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LfEventType {
    Started,
    Completed,
    Errored,
    Escalated,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LfEventFields {
    pub wave_name: Option<String>,
    pub worktree: Option<String>,
    pub command: Option<Vec<String>>,
    pub graph: Option<String>,
    pub skill: Option<String>,
    pub index: Option<u32>,
    pub error: Option<String>,
    pub signal: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LfEvent {
    pub trace_id: TraceId,
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub node: LfNode,
    pub event: LfEventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

pub fn emit(repo_root: &Path, node: LfNode, event: LfEventType, fields: LfEventFields) {
    if let Err(err) = try_emit(repo_root, node, event, fields) {
        debug!(
            error = %err,
            repo = %repo_root.display(),
            ?node,
            ?event,
            "journal append failed"
        );
    }
}

/// Own the actual CLI return, without opening a store before command admission.
///
/// A unit test that called this would make every later test in its process
/// look like the CLI, skipping admission; only the `lf` binary may.
///
/// # Panics
/// Panics if called more than once in the same process.
#[cfg(not(test))]
pub fn with_process(run: impl FnOnce() -> anyhow::Result<()>) -> anyhow::Result<()> {
    PROCESS_STARTED_AT
        .set(OffsetDateTime::now_utc().unix_timestamp())
        .expect("one lf entry point per process");
    PROCESS_START
        .set(Instant::now())
        .expect("one lf entry point per process");
    let result = run();
    crate::agent::wait_for_interrupt_cleanup();
    // A read admits no Process, but a step its Graph minted still writes its end.
    if current_context().is_none() && std::env::var_os(LF_EXECUTION_ID_ENV).is_some() {
        if let Err(error) = std::env::current_dir().and_then(|directory| {
            ensure_process_context(
                &directory,
                &LfEventFields {
                    command: Some(std::env::args().collect()),
                    worktree: Some(directory.display().to_string()),
                    ..LfEventFields::default()
                },
            )
        }) {
            warn!(%error, "this step's end is not recorded");
        }
    }
    if let Some(context) = current_context() {
        finish_runtime(&context.cwd, &result);
    }
    crate::ops::wt_timing::finish(result.is_ok());
    result
}

pub fn command_exit_code<T>(result: &anyhow::Result<T>) -> u8 {
    match result {
        Ok(_) => 0,
        Err(error) if error.is::<crate::process::GraphHeld>() => crate::process::GraphHeld::EXIT,
        Err(error) => error
            .downcast_ref::<crate::process::CommandExit>()
            .map_or(1, |exit| exit.0),
    }
}

/// Whether `error` is only an exit status, with nothing said about why.
fn is_bare_exit(error: &anyhow::Error) -> bool {
    error.chain().count() == 1 && error.is::<crate::process::CommandExit>()
}

pub(crate) fn is_cli_process() -> bool {
    PROCESS_STARTED_AT.get().is_some()
}

pub fn with_runtime<T>(
    repo_root: &Path,
    command: &[String],
    run: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    // The executable owns admission and completion, even when observation
    // failed. Nested library wrappers cannot mint another Process to recover it.
    if request::active() || is_cli_process() || current_context().is_some() {
        return run();
    }
    run_admitted(repo_root, command, run)
}

/// A library caller's Process outlives no call: one that unwinds ends
/// `Errored` with its panic, as an `lf` that panicked reads, and the unwind
/// continues to the caller.
fn run_admitted<T>(
    repo_root: &Path,
    command: &[String],
    run: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    admit_process(repo_root, command);
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(result) => {
            finish_runtime(repo_root, &result);
            result
        }
        Err(panic) => {
            let message = panic
                .downcast_ref::<&str>()
                .map(|message| (*message).to_owned())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_owned());
            emit(
                repo_root,
                LfNode::Process,
                LfEventType::Errored,
                LfEventFields {
                    error: Some(format!("panicked: {message}")),
                    exit_code: Some(PANIC_EXIT),
                    ..LfEventFields::default()
                },
            );
            std::panic::resume_unwind(panic)
        }
    }
}

/// The status a Rust program exits with when it panics.
const PANIC_EXIT: i32 = 101;

/// Attach ordinary command observation after installation/data selection.
pub fn admit_process(repo_root: &Path, command: &[String]) {
    if request::active() || EFFECT_FREE_READ.load(Ordering::Relaxed) {
        return;
    }
    if READ.load(Ordering::Relaxed) {
        if let Err(error) = open_ledger() {
            debug!(%error, "store unavailable for a read");
        }
        return;
    }
    let started = Instant::now();
    let attribution = crate::work::wave::context::process_attribution(Some(repo_root));
    if let Some(failure) = attribution.failure.as_deref() {
        warn!(
            error = failure,
            "ambient wave identity failed validation; Process attributed to no wave \
             — pass --wave <name> to recover"
        );
    }
    emit(
        repo_root,
        LfNode::Process,
        LfEventType::Started,
        LfEventFields {
            wave_name: attribution.wave,
            error: attribution.failure,
            worktree: Some(repo_root.display().to_string()),
            command: Some(command.to_vec()),
            ..LfEventFields::default()
        },
    );
    match current_context() {
        Some(context) => crate::os_process::detached::acknowledge(&context.lf_process_id),
        None => tracing::warn!("trace log unavailable: no compatible store for this process"),
    }
    tracing::debug!(
        elapsed_ms = started.elapsed().as_millis(),
        "admitted Process"
    );
}

const EXEC_PROCESS_ENV: &str = "LF_EXEC_PROCESS";

/// Keep this command's identity across a relay exec, without its open store.
pub(crate) fn carry_across_exec(command: &mut Command) -> anyhow::Result<()> {
    if let Some(context) = current_context() {
        command.env(
            EXEC_PROCESS_ENV,
            serde_json::to_string(&(std::process::id(), context))?,
        );
    }
    Ok(())
}

/// Restore the relay's command identity without opening its store. Its actual
/// return or interruption writes its end as usual.
pub fn continue_after_exec() -> anyhow::Result<()> {
    let Some(value) = std::env::var_os(EXEC_PROCESS_ENV) else {
        return Ok(());
    };
    std::env::remove_var(EXEC_PROCESS_ENV);
    let (pid, context): (u32, ProcessContext) = serde_json::from_str(
        value
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("exec process identity is not UTF-8"))?,
    )?;
    // Exec keeps the PID; a child cannot adopt its parent's identity.
    if pid == std::process::id() {
        set_context(context.clone());
        crate::agent::register_interrupt_cleanup(move || record_process_interruption(&context));
    }
    Ok(())
}

fn finish_runtime<T>(directory: &Path, result: &anyhow::Result<T>) {
    let code = command_exit_code(result);
    emit(
        directory,
        LfNode::Process,
        if code == 0 {
            LfEventType::Completed
        } else {
            LfEventType::Errored
        },
        LfEventFields {
            // A bare exit 130 is an interrupt, not an error: its run reads
            // as interrupted.
            error: result
                .as_ref()
                .err()
                .filter(|error| code != 0 && !(code == 130 && is_bare_exit(error)))
                .map(|error| format!("{error:#}")),
            exit_code: Some(i32::from(code)),
            ..LfEventFields::default()
        },
    );
}

pub fn traces_root(worktree: &Path) -> PathBuf {
    worktree.join(JOURNAL_ROOT)
}

pub fn events_path(trace_dir: &Path) -> PathBuf {
    trace_dir.join("events.jsonl")
}

pub fn read_events(trace_dir: &Path) -> Result<Vec<LfEvent>, std::io::Error> {
    let path = events_path(trace_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(path)?;
    let mut events = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str(&line).map_err(std::io::Error::other)?;
        events.push(event);
    }
    Ok(events)
}

fn try_emit(
    repo_root: &Path,
    node: LfNode,
    event: LfEventType,
    fields: LfEventFields,
) -> Result<(), std::io::Error> {
    if request::active() {
        tracing::info!(target: "loopflow::server::request", ?node, ?event, ?fields, repository = %repo_root.display(), "request effect");
        return Ok(());
    }
    let is_process_started = matches!((node, event), (LfNode::Process, LfEventType::Started));
    let maybe_context = if is_process_started {
        ensure_process_context(repo_root, &fields)?
    } else {
        current_context()
    };

    let Some(context) = maybe_context else {
        return Ok(());
    };

    let terminal = matches!(node, LfNode::Process)
        && matches!(
            event,
            LfEventType::Completed | LfEventType::Errored | LfEventType::Escalated
        );
    if terminal && context.finished.swap(true, Ordering::AcqRel) {
        return Ok(());
    }

    let exit_code = fields.exit_code;
    let event = LfEvent {
        trace_id: context.trace_id.clone(),
        ts: if is_process_started {
            OffsetDateTime::from_unix_timestamp(context.started_at)
                .map_err(std::io::Error::other)?
        } else {
            OffsetDateTime::now_utc()
        },
        node,
        event,
        wave_name: fields.wave_name,
        worktree: fields.worktree,
        command: fields.command,
        graph: fields.graph,
        skill: fields.skill,
        index: fields.index,
        error: fields.error,
        signal: fields.signal,
    };

    if let Some(trace_dir) = &context.trace_dir {
        if let Err(error) = append_event(trace_dir, &event) {
            warn!(%error, path = %trace_dir.display(), "file journal append failed; recording to ledger");
        }
    }

    record_end(&context, &event, exit_code);

    if terminal {
        if context.minted_trace_id {
            std::env::remove_var(LF_TRACE_ID_ENV);
        }
        std::env::remove_var(LF_PROCESS_ID_ENV);
        clear_context();
    }

    Ok(())
}

fn record_process_interruption(context: &ProcessContext) {
    if context.finished.swap(true, Ordering::AcqRel) {
        return;
    }
    let event = LfEvent {
        trace_id: context.trace_id.clone(),
        ts: OffsetDateTime::now_utc(),
        node: LfNode::Process,
        event: LfEventType::Escalated,
        wave_name: context.wave.clone(),
        worktree: Some(context.cwd.display().to_string()),
        command: None,
        graph: None,
        skill: None,
        index: None,
        error: None,
        signal: None,
    };
    // ctrlc's termination hook does not identify which signal arrived.
    // Keep that unknown while recording the observed interrupted exit.
    record_end(context, &event, Some(130));
}

/// What this process's own run record says at its start.
pub fn current_execution_start() -> Option<loopflow_storage::graph_process::ExecutionStart> {
    current_context().map(|context| loopflow_storage::graph_process::ExecutionStart {
        started_at: context.started_at,
        pid: Some(std::process::id()),
        os_started_at: context.process_started_at,
        caller_session: context.caller_session,
    })
}

/// The Session whose agent started this process, directly or through the
/// lf processes between.
pub fn caller_session_id() -> Option<LfSessionId> {
    current_context().and_then(|context| context.caller_session)
}

/// The repository this process records itself under: the main checkout of
/// the one it runs in.
pub fn current_repo() -> Option<String> {
    current_context().and_then(|context| context.repo)
}

/// This lf's own OS process, when `process` is this lf.
pub(crate) fn current_holder(
    process: &LfProcessId,
) -> Option<loopflow_storage::process::AttachmentHolder> {
    current_context()
        .filter(|context| &context.lf_process_id == process)
        .map(|context| loopflow_storage::process::AttachmentHolder {
            pid: Some(std::process::id()),
            os_started_at: context.process_started_at,
        })
}

/// A run writes its own end on the records it owns, at its terminal event:
/// the step its Graph minted it for, and any Graph run, Task run or Project
/// activation it recorded. A command that owns none writes nothing. Never
/// fails the run: the first failure warns, later ones log at debug.
fn record_end(context: &ProcessContext, event: &LfEvent, exit_code: Option<i32>) {
    if event.node != LfNode::Process {
        return;
    }
    let exit_code = exit_code.or(match event.event {
        LfEventType::Completed => Some(0),
        LfEventType::Errored => Some(1),
        LfEventType::Escalated => Some(130),
        LfEventType::Started => return,
    });
    let owners = context
        .run_owners
        .lock()
        .expect("run owner mutex poisoned")
        .clone();
    if owners.is_empty() {
        return;
    }
    let store = match context.ledger.get().cloned().map_or_else(
        || {
            let store = crate::store::open_sqlite(&context.ledger_path)?;
            let _ = context.ledger.set(store.clone());
            Ok::<_, crate::store::StoreError>(store)
        },
        Ok,
    ) {
        Ok(store) => store,
        Err(error) => {
            if first_ledger_failure() {
                warn!(%error, "store unavailable — this run's end is not recorded");
            } else {
                debug!(%error, "store unavailable");
            }
            return;
        }
    };
    let ended_at = event.ts.unix_timestamp();
    for owner in owners {
        if let Err(error) = store.finish_run(
            owner,
            &context.lf_process_id,
            ended_at,
            exit_code,
            event.error.as_deref(),
        ) {
            if first_ledger_failure() {
                warn!(%error, ?owner, "could not record this run's end on its owner record");
            } else {
                debug!(%error, ?owner, "could not record this run's end");
            }
        }
    }
}

/// This process recorded itself on `owner` under its own id and writes its
/// end there.
pub fn mark_run_owner(owner: RunOwner) {
    if let Some(context) = current_context() {
        let mut owners = context.run_owners.lock().expect("run owner mutex poisoned");
        if !owners.contains(&owner) {
            owners.push(owner);
        }
    }
}

/// True exactly once per process. A ledger write must never fail a run, but a
/// silent best-effort write turns a schema break into invisible data loss: a
/// `step_index`/`skill_index` drift once cost 29 hours of run history while
/// every reader failed loudly and every writer whispered at `debug!`. Say it
/// once, at a level someone runs; stay quiet after so a broken ledger does not
/// drown the run's own output.
fn first_ledger_failure() -> bool {
    static WARNED: AtomicBool = AtomicBool::new(false);
    !WARNED.swap(true, Ordering::Relaxed)
}

/// Open the local ledger store, creating and migrating it if needed.
pub fn open_ledger() -> Result<SqliteStore, crate::store::StoreError> {
    crate::store::open_sqlite(&ledger_db_path()?)
}

/// Whether the ledger exists, so a read that could only find nothing can
/// skip creating it.
pub(crate) fn ledger_exists() -> bool {
    ledger_db_path().is_ok_and(|path| path.exists())
}

/// This Process's retained store connection, opening one when it has none;
/// every open revalidates the schema.
pub(crate) fn process_ledger() -> Result<SqliteStore, crate::store::StoreError> {
    match current_context().and_then(|context| context.ledger.get().cloned()) {
        Some(store) => Ok(store),
        None => open_ledger(),
    }
}

fn ledger_db_path() -> Result<PathBuf, crate::store::StoreError> {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(path) = TEST_LEDGER_DB_PATH.with(|path| path.borrow().clone()) {
        return Ok(path);
    }
    #[cfg(test)]
    {
        static TEST_HOME: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
        Ok(TEST_HOME
            .get_or_init(|| tempfile::TempDir::new().expect("test ledger home"))
            .path()
            .join("loopflow.db"))
    }
    #[cfg(not(test))]
    crate::store::database_path_from_env()
        .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
}

fn ensure_process_context(
    repo_root: &Path,
    fields: &LfEventFields,
) -> Result<Option<ProcessContext>, std::io::Error> {
    if let Some(context) = current_context() {
        return Ok(Some(context));
    }

    create_process_context(
        repo_root,
        fields,
        ledger_db_path().map_err(std::io::Error::other)?,
    )
}

fn create_process_context(
    repo_root: &Path,
    fields: &LfEventFields,
    ledger_path: PathBuf,
) -> Result<Option<ProcessContext>, std::io::Error> {
    let main_repo = main_repo_root(repo_root).ok();
    let attribution = crate::work::wave::context::process_attribution(main_repo.as_deref());
    if let Some(failure) = attribution.failure.as_deref() {
        debug!(
            error = failure,
            "ambient wave identity failed validation; Process has no wave"
        );
    }
    let wave_name = attribution.wave;

    let agent_caller = std::env::var_os(AGENT_CALLER_ENV)
        .map(|value| {
            let value = value
                .into_string()
                .map_err(|_| std::io::Error::other("agent caller is not valid UTF-8"))?;
            serde_json::from_str::<AgentCaller>(&value).map_err(std::io::Error::other)
        })
        .transpose()?;
    // A direct child of this lf process must inherit this Process, not the agent
    // edge that admitted it. Provider launches install their own fresh caller.
    std::env::remove_var(AGENT_CALLER_ENV);
    let inherited_trace = match agent_caller.as_ref() {
        Some(caller) => caller.trace_id.clone(),
        None => configured_trace_id(repo_root),
    };
    let (trace_id, minted_trace_id) = match inherited_trace {
        Some(trace_id) => (trace_id, false),
        None => (TraceId::default(), true),
    };

    // A parent process id only means "my parent within this trace": a fresh
    // trace id makes a lingering LF_PROCESS_ID belong to the old trace.
    let parent_lf_process_id = if let Some(caller) = agent_caller.as_ref() {
        (!minted_trace_id).then(|| caller.origin_lf_process_id.clone())
    } else {
        (!minted_trace_id)
            .then(|| {
                std::env::var(LF_PROCESS_ID_ENV)
                    .ok()
                    .and_then(|value| LfProcessId::parse(&value).ok())
            })
            .flatten()
    };
    let minted = std::env::var(LF_EXECUTION_ID_ENV)
        .ok()
        .and_then(|value| LfProcessId::parse(&value).ok());
    std::env::remove_var(LF_EXECUTION_ID_ENV);
    // A Graph mints its steps' ids; a Task run mints its Graph run's, which
    // that run marks once it records itself.
    let run_owners = Arc::new(Mutex::new(
        minted
            .as_ref()
            .filter(|_| graph_step() || std::env::var_os(GRAPH_STEP_ENV).is_some())
            .map(|_| RunOwner::GraphStep)
            .into_iter()
            .collect(),
    ));
    let lf_process_id = minted.unwrap_or_default();
    // Export the identity so prompt logs and child processes carry the same
    // trace and parent. The export is removed when the
    // Process ends (see try_emit).
    std::env::set_var(LF_TRACE_ID_ENV, trace_id.as_str());
    std::env::set_var(LF_PROCESS_ID_ENV, lf_process_id.as_str());
    let caller_session = agent_caller
        .as_ref()
        .map(|caller| caller.session_id.clone())
        .or_else(|| {
            std::env::var(LF_CALLER_SESSION_ENV)
                .ok()
                .map(LfSessionId::from)
        });
    match &caller_session {
        Some(session) => std::env::set_var(LF_CALLER_SESSION_ENV, session.as_str()),
        None => std::env::remove_var(LF_CALLER_SESSION_ENV),
    }

    // Write the file journal wherever the trace directory can be
    // git-excluded (not outside a repository).
    let trace_dir = match crate::repo::discover_repo_root(repo_root)
        .map_err(std::io::Error::other)
        .and_then(|root| {
            root.ok_or_else(|| std::io::Error::other("no repository for file journal"))
        })
        .and_then(|root| {
            crate::git::exclude(&root, JOURNAL_EXCLUDE_ENTRY)?;
            let dir = traces_root(&root).join(trace_id.as_str());
            fs::create_dir_all(&dir)?;
            Ok(dir)
        }) {
        Ok(dir) => Some(dir),
        Err(err) => {
            debug!(
                error = %err,
                repo = %repo_root.display(),
                "file journal unavailable"
            );
            None
        }
    };

    let repo = main_repo
        .as_deref()
        .unwrap_or(repo_root)
        .display()
        .to_string();

    let process_started_at = crate::process::os_process::process_started_at(std::process::id())
        .unwrap_or_else(|error| {
            debug!(%error, "Process evidence unavailable; recording command history only");
            None
        });
    let context = ProcessContext {
        trace_id,
        lf_process_id,
        parent_lf_process_id,
        agent_caller,
        caller_session,
        started_at: PROCESS_STARTED_AT
            .get()
            .copied()
            .unwrap_or_else(|| OffsetDateTime::now_utc().unix_timestamp()),
        process_started_at,
        cwd: repo_root.to_path_buf(),
        ledger_path,
        ledger: Arc::default(),
        command: fields
            .command
            .as_ref()
            .and_then(|argv| serde_json::to_string(argv).ok()),
        trace_dir,
        repo: Some(repo),
        wave: wave_name.clone(),
        finished: Arc::new(AtomicBool::new(false)),
        minted_trace_id,
        run_owners,
    };
    set_context(context.clone());
    // Only an `lf` executable exits on interrupt; library callers settle their
    // runtime when the call returns rather than registering process-wide cleanup.
    if is_cli_process() {
        let interrupted = context.clone();
        crate::agent::register_interrupt_cleanup(move || record_process_interruption(&interrupted));
    }

    if let Some(wave_name) = wave_name {
        if fields.wave_name.as_deref() != Some(wave_name.as_str()) {
            debug!(
                expected_wave = %wave_name,
                observed_wave = ?fields.wave_name,
                repo = %repo_root.display(),
                "Process start received mismatched wave metadata"
            );
        }
    }

    Ok(Some(context))
}

fn configured_trace_id(repo_root: &Path) -> Option<TraceId> {
    let value = std::env::var(LF_TRACE_ID_ENV).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    match trimmed.parse() {
        Ok(trace_id) => Some(trace_id),
        Err(err) => {
            debug!(
                env = LF_TRACE_ID_ENV,
                value = trimmed,
                repo = %repo_root.display(),
                error = %err,
                "ignoring invalid journal trace id override"
            );
            None
        }
    }
}

fn append_event(trace_dir: &Path, event: &LfEvent) -> Result<(), std::io::Error> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_path(trace_dir))?;
    let _lock = lock_file(&file)?;
    let mut line = serde_json::to_vec(event).map_err(std::io::Error::other)?;
    line.push(b'\n');
    file.write_all(&line)?;
    Ok(())
}

/// Concurrent processes append whole lines to a trace's event log.
#[cfg(unix)]
struct FileLock {
    fd: std::os::fd::RawFd,
}

#[cfg(unix)]
fn lock_file(file: &File) -> Result<FileLock, std::io::Error> {
    let fd = file.as_raw_fd();
    loop {
        // SAFETY: flock only observes the valid file descriptor borrowed from
        // `file`; the File outlives the returned guard.
        if unsafe { libc::flock(fd, libc::LOCK_EX) } == 0 {
            return Ok(FileLock { fd });
        }
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

#[cfg(unix)]
impl Drop for FileLock {
    fn drop(&mut self) {
        // SAFETY: the guard only exists while the borrowed File is alive.
        let _ = unsafe { libc::flock(self.fd, libc::LOCK_UN) };
    }
}

#[cfg(not(unix))]
struct FileLock;

#[cfg(not(unix))]
fn lock_file(_file: &File) -> Result<FileLock, std::io::Error> {
    Ok(FileLock)
}

fn current_context() -> Option<ProcessContext> {
    if request::active() {
        return None;
    }
    if is_cli_process() {
        return PROCESS_CONTEXT
            .lock()
            .expect("process context mutex poisoned")
            .clone();
    }
    THREAD_CONTEXT.with(|cell| cell.borrow().clone())
}

/// Caller provenance captured at process entry, before the environment is consumed.
/// The agent edge that issued this command: its Process's, or before
/// admission (`lf open` admits none) the one it inherited.
pub fn agent_caller() -> Option<AgentCaller> {
    match current_context() {
        Some(context) => context.agent_caller,
        None => std::env::var(AGENT_CALLER_ENV)
            .ok()
            .and_then(|value| serde_json::from_str(&value).ok()),
    }
}

/// A nested command leaves checkpoint composition to its caller.
pub fn has_caller() -> bool {
    current_context().is_some_and(|context| {
        context.agent_caller.is_some() || context.parent_lf_process_id.is_some()
    })
}

/// This process's argv as JSON, as recorded when it started.
pub(crate) fn current_command() -> Option<String> {
    current_context().and_then(|context| context.command)
}

pub fn current_lf_process_id() -> Option<LfProcessId> {
    current_context().map(|context| context.lf_process_id)
}

/// The lf this process runs under, as it inherited it.
pub fn current_parent_lf_process_id() -> Option<LfProcessId> {
    current_context().and_then(|context| context.parent_lf_process_id)
}

/// This process's trace, which the agents it launches join.
pub fn current_trace_id() -> Option<TraceId> {
    current_context().map(|context| context.trace_id)
}

fn set_context(context: ProcessContext) {
    if is_cli_process() {
        *PROCESS_CONTEXT
            .lock()
            .expect("process context mutex poisoned") = Some(context);
        return;
    }
    THREAD_CONTEXT.with(|cell| {
        *cell.borrow_mut() = Some(context);
    });
}

fn clear_context() {
    if is_cli_process() {
        *PROCESS_CONTEXT
            .lock()
            .expect("process context mutex poisoned") = None;
        return;
    }
    THREAD_CONTEXT.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

#[cfg(test)]
mod tests {
    use super::{
        emit, events_path, read_events, traces_root, LfEvent, LfEventFields, LfEventType, LfNode,
        TestLedgerGuard,
    };
    use crate::git::is_clean;
    use loopflow_model::id::{LfProcessId, TraceId};
    use loopflow_test_support::TestRepo;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    const CHILD_APPEND_ENV: &str = "LOOPFLOW_JOURNAL_APPEND_CHILD";
    const CHILD_EVENT_COUNT_ENV: &str = "LOOPFLOW_JOURNAL_CHILD_EVENT_COUNT";
    const CHILD_TRACE_DIR_ENV: &str = "LOOPFLOW_JOURNAL_CHILD_TRACE_DIR";
    const CHILD_WRITER_ENV: &str = "LOOPFLOW_JOURNAL_CHILD_WRITER";

    struct AmbientStorage {
        _lock: std::sync::MutexGuard<'static, ()>,
        previous_lf_home: Option<std::ffi::OsString>,
    }

    impl AmbientStorage {
        fn seed(home: &std::path::Path) -> Self {
            let lock = super::test_env_lock();
            let previous_lf_home = std::env::var_os("LF_HOME");
            std::env::set_var("LF_HOME", home);
            Self {
                _lock: lock,
                previous_lf_home,
            }
        }
    }

    impl Drop for AmbientStorage {
        fn drop(&mut self) {
            match &self.previous_lf_home {
                Some(value) => std::env::set_var("LF_HOME", value),
                None => std::env::remove_var("LF_HOME"),
            }
        }
    }

    fn with_trace_id_env<T>(value: Option<&str>, run: impl FnOnce() -> T) -> T {
        let _guard = journal_test_guard();
        super::clear_context();
        let previous = std::env::var(super::LF_TRACE_ID_ENV).ok();
        let previous_process = std::env::var(super::LF_PROCESS_ID_ENV).ok();
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        match value {
            Some(value) => std::env::set_var(super::LF_TRACE_ID_ENV, value),
            None => std::env::remove_var(super::LF_TRACE_ID_ENV),
        }
        let result = run();
        super::clear_context();
        match previous {
            Some(value) => std::env::set_var(super::LF_TRACE_ID_ENV, value),
            None => std::env::remove_var(super::LF_TRACE_ID_ENV),
        }
        match previous_process {
            Some(value) => std::env::set_var(super::LF_PROCESS_ID_ENV, value),
            None => std::env::remove_var(super::LF_PROCESS_ID_ENV),
        }
        result
    }

    fn journal_test_guard() -> TestLedgerGuard {
        let guard = TestLedgerGuard::new();
        super::clear_context();
        std::env::remove_var(super::LF_TRACE_ID_ENV);
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        guard
    }

    #[test]
    fn explicit_test_database_path_controls_the_ledger() {
        let guard = journal_test_guard();
        let path = guard.home().join("explicit.db");
        guard.set_db_path(path.clone());

        let opened = super::open_ledger();

        opened.expect("open explicit ledger");
        assert!(path.exists());
    }

    #[test]
    fn unit_test_ledger_ignores_ambient_storage_paths() {
        let ambient_home = tempfile::tempdir().expect("ambient home");
        let _ambient = AmbientStorage::seed(ambient_home.path());

        let resolved = super::ledger_db_path().expect("test ledger path");
        super::open_ledger().expect("open test ledger");

        assert_ne!(resolved, ambient_home.path().join("loopflow.db"));
        assert!(!ambient_home.path().join("loopflow.db").exists());
    }

    fn started_fields(
        command: &[String],
        worktree: &std::path::Path,
        wave_name: &str,
    ) -> LfEventFields {
        LfEventFields {
            wave_name: Some(wave_name.to_string()),
            worktree: Some(worktree.display().to_string()),
            command: Some(command.to_vec()),
            ..LfEventFields::default()
        }
    }

    fn only_trace_dir(worktree: &std::path::Path) -> std::path::PathBuf {
        let mut entries = std::fs::read_dir(traces_root(worktree))
            .expect("read traces")
            .map(|entry| entry.expect("trace dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1, "expected a single journal trace dir");
        entries.pop().expect("trace dir")
    }

    #[test]
    fn concurrent_child_process_appends_keep_events_jsonl_parseable() {
        let tmp = tempfile::TempDir::new().expect("temp journal");
        let trace_dir = tmp.path().join("trace");
        std::fs::create_dir_all(&trace_dir).expect("create trace dir");

        let current_exe = std::env::current_exe().expect("current test binary");
        let writers = 8;
        let events_per_writer = 20;
        let mut children = Vec::new();
        for writer in 0..writers {
            let child = Command::new(&current_exe)
                .arg("journal_child_process_appends_events_for_concurrency_regression")
                .arg("--nocapture")
                .env(CHILD_APPEND_ENV, "1")
                .env(CHILD_TRACE_DIR_ENV, &trace_dir)
                .env(CHILD_WRITER_ENV, writer.to_string())
                .env(CHILD_EVENT_COUNT_ENV, events_per_writer.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn child journal writer");
            children.push(child);
        }

        for mut child in children {
            let status = child.wait().expect("wait for child journal writer");
            assert!(status.success(), "child journal writer failed: {status}");
        }

        let raw = std::fs::read_to_string(events_path(&trace_dir)).expect("read events.jsonl");
        let lines = raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), writers * events_per_writer);

        for (line_number, line) in lines.iter().enumerate() {
            serde_json::from_str::<LfEvent>(line).unwrap_or_else(|err| {
                panic!("line {} is malformed JSONL: {err}: {line}", line_number + 1)
            });
        }

        let parsed = read_events(&trace_dir).expect("parse all events");
        assert_eq!(parsed.len(), writers * events_per_writer);
    }

    #[test]
    fn journal_child_process_appends_events_for_concurrency_regression() {
        if std::env::var(CHILD_APPEND_ENV).ok().as_deref() != Some("1") {
            return;
        }

        let trace_dir =
            PathBuf::from(std::env::var(CHILD_TRACE_DIR_ENV).expect("child trace dir env"));
        let writer = std::env::var(CHILD_WRITER_ENV).expect("child writer env");
        let event_count = std::env::var(CHILD_EVENT_COUNT_ENV)
            .expect("child event count env")
            .parse::<usize>()
            .expect("child event count");
        let trace_id = TraceId::parse("8985c55b-9864-4c2b-860f-b7054a71bbea").expect("trace id");

        for index in 0..event_count {
            let event = LfEvent {
                trace_id: trace_id.clone(),
                ts: time::OffsetDateTime::now_utc(),
                node: LfNode::Skill,
                event: LfEventType::Errored,
                wave_name: Some("meta".to_string()),
                worktree: Some(trace_dir.display().to_string()),
                command: None,
                graph: Some("garden".to_string()),
                skill: Some(format!("writer-{writer}-{index}")),
                index: Some(index as u32),
                error: Some(format!(
                    "writer-{writer}-event-{index}:{}",
                    "x".repeat(16 * 1024)
                )),
                signal: None,
            };
            super::append_event(&trace_dir, &event).expect("child append event");
        }
    }

    #[test]
    fn a_nested_lf_gets_its_own_span_and_names_its_parent() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let fields = started_fields(&["lf".to_string(), "wave".to_string()], repo.path(), "main");

        // The parent emits, which puts its identity in the environment.
        super::emit(
            repo.path(),
            LfNode::Process,
            LfEventType::Started,
            fields.clone(),
        );
        let parent = super::current_context().expect("parent");
        assert!(!super::has_caller());
        super::clear_context();
        let child = super::ensure_process_context(repo.path(), &fields)
            .expect("child context")
            .expect("child");

        assert!(super::has_caller());
        assert_ne!(parent.lf_process_id, child.lf_process_id);
        assert_eq!(
            child.trace_id, parent.trace_id,
            "a nested lf stays in the trace"
        );
        assert_eq!(child.parent_lf_process_id, Some(parent.lf_process_id));
        super::clear_context();
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        std::env::remove_var(super::LF_TRACE_ID_ENV);
    }

    #[test]
    fn minting_a_fresh_trace_drops_a_stale_cross_trace_parent() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let fields = started_fields(
            &["lf".to_string(), "kickoff".to_string()],
            repo.path(),
            "main",
        );

        // A process id lingers in the environment but no trace id does — the
        // `pr land` / `wt switch` / `kickoff` shape that historically stamped a
        // new trace with a parent from the old one.
        std::env::set_var(super::LF_PROCESS_ID_ENV, LfProcessId::new().as_str());

        let context = super::ensure_process_context(repo.path(), &fields)
            .expect("Process context")
            .expect("context");

        assert!(
            context.minted_trace_id,
            "no LF_TRACE_ID means a fresh trace"
        );
        assert_eq!(
            context.parent_lf_process_id, None,
            "a fresh trace has no in-trace parent to name"
        );
        super::clear_context();
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        std::env::remove_var(super::LF_TRACE_ID_ENV);
    }

    #[test]
    fn journal_writes_process_graph_and_skill_events_in_wave_worktree() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let worktree = repo.create_named_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Process,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        emit(
            &worktree,
            LfNode::Graph,
            LfEventType::Started,
            LfEventFields {
                graph: Some("build".to_string()),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Skill,
            LfEventType::Started,
            LfEventFields {
                skill: Some("implement".to_string()),
                index: Some(0),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Skill,
            LfEventType::Completed,
            LfEventFields {
                skill: Some("implement".to_string()),
                index: Some(0),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Graph,
            LfEventType::Completed,
            LfEventFields::default(),
        );
        emit(
            &worktree,
            LfNode::Process,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        let trace_dir = only_trace_dir(&worktree);
        let events = read_events(&trace_dir).expect("read events");
        assert_eq!(events.len(), 6);
        assert_eq!(events[0].node, LfNode::Process);
        assert_eq!(events[0].event, LfEventType::Started);
        assert_eq!(events[0].wave_name.as_deref(), Some("runtime"));
        assert_eq!(events[1].node, LfNode::Graph);
        assert_eq!(events[1].graph.as_deref(), Some("build"));
        assert_eq!(events[2].node, LfNode::Skill);
        assert_eq!(events[2].skill.as_deref(), Some("implement"));
        assert_eq!(events[2].index, Some(0));
        assert_eq!(events[3].event, LfEventType::Completed);
        assert_eq!(events[4].node, LfNode::Graph);
        assert_eq!(events[5].node, LfNode::Process);
        assert_eq!(events[5].event, LfEventType::Completed);
    }

    #[test]
    fn journal_keeps_worktree_clean() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let worktree = repo.create_named_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Process,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );

        assert!(is_clean(&worktree).expect("worktree should stay clean"));
    }

    #[test]
    fn terminal_process_events_clear_context_for_the_next_process() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let worktree = repo.create_named_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Process,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        emit(
            &worktree,
            LfNode::Process,
            LfEventType::Completed,
            LfEventFields::default(),
        );
        emit(
            &worktree,
            LfNode::Process,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        emit(
            &worktree,
            LfNode::Process,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        let entries = std::fs::read_dir(traces_root(&worktree))
            .expect("read traces")
            .count();
        assert_eq!(entries, 2);
    }

    #[test]
    fn journal_uses_configured_trace_id_when_present() {
        with_trace_id_env(Some("7c22895f-e4c1-49cc-a95d-2267e2356f16"), || {
            let repo = TestRepo::new();
            let worktree = repo.create_named_worktree("runtime");
            let command = vec!["lf".to_string(), "build".to_string()];

            emit(
                &worktree,
                LfNode::Process,
                LfEventType::Started,
                started_fields(&command, &worktree, "runtime"),
            );
            emit(
                &worktree,
                LfNode::Process,
                LfEventType::Completed,
                LfEventFields::default(),
            );

            let trace_dir = only_trace_dir(&worktree);
            let trace_id = trace_dir
                .file_name()
                .and_then(|name| name.to_str())
                .expect("trace dir name");
            assert_eq!(trace_id, "7c22895f-e4c1-49cc-a95d-2267e2356f16");
        });
    }

    #[test]
    fn invalid_configured_trace_id_falls_back_to_generated_id() {
        with_trace_id_env(Some("not-a-uuid"), || {
            let repo = TestRepo::new();
            let worktree = repo.create_named_worktree("runtime");
            let command = vec!["lf".to_string(), "build".to_string()];

            emit(
                &worktree,
                LfNode::Process,
                LfEventType::Started,
                started_fields(&command, &worktree, "runtime"),
            );
            emit(
                &worktree,
                LfNode::Process,
                LfEventType::Completed,
                LfEventFields::default(),
            );

            let trace_dir = only_trace_dir(&worktree);
            let trace_id = trace_dir
                .file_name()
                .and_then(|name| name.to_str())
                .expect("trace dir name");
            assert!(
                TraceId::parse(trace_id).is_ok(),
                "expected generated UUID trace id"
            );
            assert_ne!(trace_id, "not-a-uuid");
        });
    }
}
