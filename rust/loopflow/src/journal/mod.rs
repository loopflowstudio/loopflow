mod os_process;
pub(crate) use os_process::OsProcess;

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
use crate::id::{LfProcessId, TraceId};
use crate::process::{AgentCaller, LfProcess, AGENT_CALLER_ENV};
use crate::store::sqlite::SqliteStore;

const JOURNAL_ROOT: &str = ".lf/journal/traces";
const JOURNAL_EXCLUDE_ENTRY: &str = ".lf/journal/";
pub(crate) const PROCESS_RECEIPT_ROOT: &str = "runtime/exec-processes";
pub const LF_TRACE_ID_ENV: &str = "LF_TRACE_ID";
pub const LF_PROCESS_ID_ENV: &str = "LF_PROCESS_ID";

/// Serializes tests that mutate process-global store or Process identity variables.
/// Every test in the crate that touches these variables must hold this lock.
#[cfg(test)]
pub(crate) fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    loopflow_test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
thread_local! {
    static TEST_LEDGER_DB_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct TestLedgerGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous_lf_home: Option<std::ffi::OsString>,
    previous_test_path: Option<PathBuf>,
    home: tempfile::TempDir,
}

#[cfg(test)]
impl TestLedgerGuard {
    pub(crate) fn new() -> Self {
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

    pub(crate) fn home(&self) -> &Path {
        self.home.path()
    }

    pub(crate) fn set_db_path(&self, path: PathBuf) {
        TEST_LEDGER_DB_PATH.with(|current| *current.borrow_mut() = Some(path));
    }
}

/// Pin a blocking fixture worker to its owning test's store; the caller holds
/// the environment lock. Production processes already share one admitted ledger.
#[cfg(test)]
pub(crate) fn with_test_ledger<T>(path: PathBuf, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<PathBuf>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_LEDGER_DB_PATH.with(|path| *path.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(TEST_LEDGER_DB_PATH.with(|current| current.replace(Some(path))));
    run()
}

#[cfg(test)]
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

#[derive(Debug, Clone)]
struct ProcessContext {
    trace_id: TraceId,
    lf_process_id: LfProcessId,
    parent_lf_process_id: Option<LfProcessId>,
    agent_caller: Option<AgentCaller>,
    /// Time this command entered the runtime, independent of OS inspection.
    started_at: i64,
    process_started_at: Option<i64>,
    cwd: PathBuf,
    ledger_path: PathBuf,
    /// Retain the first successful connection through Process completion.
    ledger: Arc<OnceLock<SqliteStore>>,
    receipt_wait: Duration,
    /// Serialized argv captured at Process start so terminal rows name their work.
    command: Option<String>,
    /// Optional repository trace journal. Early process observation uses only
    /// SQLite; ordinary dispatch also writes this when Git exclusion succeeds.
    trace_dir: Option<PathBuf>,
    repo: Option<String>,
    wave: Option<String>,
    finished: Arc<AtomicBool>,
    /// True when this process minted the trace id (vs inheriting LF_TRACE_ID);
    /// the export is removed again when the Process ends.
    minted_trace_id: bool,
    receipts: Arc<Mutex<ReceiptCost>>,
}

/// What a process's ledger receipts cost, and how many did not land.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReceiptCost {
    pub waited: Duration,
    pub unrecorded: u32,
}

/// How long one Process waits for a contended store across all of its receipts.
/// A start receipt that used the whole wait leaves its finish receipt a single
/// attempt, so a held write lock delays a command once, not once per receipt.
#[cfg(not(test))]
fn receipt_wait() -> Duration {
    crate::store::sqlite::SQLITE_WRITE_BUSY_TIMEOUT
}

#[cfg(test)]
thread_local! {
    static TEST_RECEIPT_WAIT: std::cell::Cell<Duration> =
        const { std::cell::Cell::new(crate::store::sqlite::SQLITE_WRITE_BUSY_TIMEOUT) };
}

#[cfg(test)]
fn receipt_wait() -> Duration {
    TEST_RECEIPT_WAIT.with(std::cell::Cell::get)
}

/// Time since this lf process entered its entry point; `None` in a library caller.
pub(crate) fn process_elapsed() -> Option<Duration> {
    PROCESS_START.get().map(Instant::elapsed)
}

/// Exact, process-local ownership evidence for a live Loopflow Process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProcessReceipt {
    pub schema_version: u32,
    pub trace_id: String,
    #[serde(rename = "exec_id")] // Persisted receipt format survives upgrades.
    pub lf_process_id: String,
    pub pid: u32,
    pub started_at: i64,
}

impl ProcessReceipt {
    pub(crate) fn matches_process(&self, process: &str, trace: &str) -> bool {
        self.lf_process_id.eq_ignore_ascii_case(process)
            && self.trace_id.eq_ignore_ascii_case(trace)
    }

    pub(crate) fn process_evidence(&self) -> ProcessIdentityEvidence {
        process_identity_evidence(self.pid, self.started_at)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessIdentityEvidence {
    Live,
    Dead,
    Unknown,
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
/// # Panics
/// Panics if called more than once in the same process.
pub fn with_process(run: impl FnOnce() -> anyhow::Result<()>) -> anyhow::Result<()> {
    PROCESS_STARTED_AT
        .set(OffsetDateTime::now_utc().unix_timestamp())
        .expect("one lf entry point per process");
    PROCESS_START
        .set(Instant::now())
        .expect("one lf entry point per process");
    let result = run();
    // Nothing can reference a Process no one saw running, so exit observation
    // writes only a row the rule may keep: one an agent issued, or one a
    // running lf parent may still read.
    if std::env::var_os(AGENT_CALLER_ENV).is_some() || std::env::var_os(LF_PROCESS_ID_ENV).is_some()
    {
        observe_process(&std::env::args().collect::<Vec<_>>());
    }
    crate::agent::wait_for_interrupt_cleanup();
    let receipts = if let Some(context) = current_context() {
        finish_runtime(&context.cwd, &result);
        let cost = *context
            .receipts
            .lock()
            .expect("receipt cost mutex poisoned");
        Some(cost)
    } else {
        None
    };
    crate::ops::wt_timing::finish(result.is_ok(), receipts);
    result
}

/// Observe an early command without creating a Machine, migrating, or requiring Git.
/// Failure to observe never prevents help or installation recovery.
pub fn observe_process(command: &[String]) {
    let wait = receipt_wait();
    // Failed early observation must not be retried at exit with a longer wait.
    static ATTEMPTED: AtomicBool = AtomicBool::new(false);
    if current_context().is_some()
        || EFFECT_FREE_READ.load(Ordering::Relaxed)
        || ATTEMPTED.swap(true, Ordering::Relaxed)
    {
        return;
    }
    let observe = || -> anyhow::Result<()> {
        let path = crate::store::database_path_from_env()?;
        let ledger = SqliteStore::open_existing_processes(&path, wait)?;
        let directory = std::env::current_dir()?;
        let fields = LfEventFields {
            command: Some(command.to_vec()),
            worktree: Some(directory.display().to_string()),
            ..LfEventFields::default()
        };
        create_process_context(&directory, &fields, path, Some(ledger), wait)?;
        try_emit(&directory, LfNode::Process, LfEventType::Started, fields)?;
        Ok(())
    };
    if let Err(error) = observe() {
        debug!(%error, "early Process observation unavailable");
    }
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
    if is_cli_process() || current_context().is_some() {
        return run();
    }
    admit_process(repo_root, command);
    let result = run();
    finish_runtime(repo_root, &result);
    result
}

/// Attach ordinary command observation after installation/data selection.
pub fn admit_process(repo_root: &Path, command: &[String]) {
    if EFFECT_FREE_READ.load(Ordering::Relaxed) {
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
    if current_context().is_none() {
        eprintln!("Process history unavailable: no compatible process ledger for this process");
    }
    tracing::debug!(
        elapsed_ms = started.elapsed().as_millis(),
        "admitted Process"
    );
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
            error: result
                .as_ref()
                .err()
                .filter(|_| code != 0)
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

    ledger_insert(&context, &event, exit_code);

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
    ledger_insert(context, &event, Some(130));
}

/// This Process's row as its start receipt writes it. Attachment writes the
/// same row in its own transaction, so an agent never names an unrecorded parent.
fn start_record(context: &ProcessContext) -> LfProcess {
    LfProcess {
        kind: crate::process::ProcessKind::Lf,
        agent_session_id: None,
        os_started_at: context.process_started_at,
        id: context.lf_process_id.clone(),
        pid: Some(std::process::id()),
        trace_id: context.trace_id.clone(),
        parent_lf_process_id: context.parent_lf_process_id.clone(),
        via_agent: Some(context.agent_caller.is_some()),
        caller_session_id: context
            .agent_caller
            .as_ref()
            .map(|caller| caller.session_id.clone()),
        caller_agent_process_id: context
            .agent_caller
            .as_ref()
            .and_then(|caller| caller.agent_process_id.clone()),
        command: context.command.clone(),
        repo: context.repo.clone(),
        cwd: Some(context.cwd.display().to_string()),
        started_at: context.started_at,
        completed_at: None,
        outcome: None,
        exit_code: None,
        signal: None,
        error: None,
    }
}

/// The current Process's start row, when `process` is the current Process.
pub(crate) fn current_start_record(process: &LfProcessId) -> Option<LfProcess> {
    current_context()
        .filter(|context| &context.lf_process_id == process)
        .map(|context| start_record(&context))
}

/// Best-effort write into the machine-grain SQLite ledger. Never fails the
/// run: the first failure warns, and later failures log at debug. Local-only —
/// the ledger never leaves the machine.
fn ledger_insert(context: &ProcessContext, event: &LfEvent, exit_code: Option<i32>) {
    if event.node != LfNode::Process {
        return;
    }
    let outcome = match event.event {
        LfEventType::Completed => Some("succeeded"),
        LfEventType::Errored => Some("failed"),
        LfEventType::Escalated => Some("interrupted"),
        _ => None,
    };
    let mut record = start_record(context);
    record.completed_at = outcome.map(|_| event.ts.unix_timestamp());
    record.outcome = outcome.map(str::to_owned);
    record.exit_code = exit_code.or(match event.event {
        LfEventType::Completed => Some(0),
        LfEventType::Errored => Some(1),
        LfEventType::Escalated => Some(130),
        _ => None,
    });
    record.signal = event.signal.clone();
    record.error = event.error.clone();

    let started = Instant::now();
    let already_waited = context
        .receipts
        .lock()
        .expect("receipt cost mutex poisoned")
        .waited;
    let recorded = match context.ledger.get().cloned().map_or_else(
        || {
            let store = SqliteStore::new(&context.ledger_path)?;
            let _ = context.ledger.set(store.clone());
            Ok::<_, crate::store::StoreError>(store)
        },
        Ok,
    ) {
        Ok(store) => {
            let wait = context
                .receipt_wait
                .saturating_sub(already_waited + started.elapsed());
            match store.record_process_within(&record, wait) {
                Ok(()) => true,
                Err(err) => {
                    let waited = (already_waited + started.elapsed()).as_secs_f64();
                    if first_ledger_failure() {
                        warn!(error = %err, trace_id = %record.trace_id, waited_seconds = waited, "ledger insert failed — this Process is not being recorded");
                    } else {
                        debug!(error = %err, trace_id = %record.trace_id, waited_seconds = waited, "ledger insert failed");
                    }
                    false
                }
            }
        }
        Err(err) => {
            if first_ledger_failure() {
                warn!(error = %err, "ledger unavailable — Processes are not being recorded");
            } else {
                debug!(error = %err, "ledger unavailable");
            }
            false
        }
    };
    // Both ordinary completion and interrupt cleanup retain identity until settlement.
    if recorded && record.completed_at.is_some() {
        remove_process_receipt(context);
    }
    let mut cost = context
        .receipts
        .lock()
        .expect("receipt cost mutex poisoned");
    cost.waited += started.elapsed();
    if !recorded {
        cost.unrecorded += 1;
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
    SqliteStore::new(&ledger_db_path()?)
}

/// This Process's retained connection to the store at `path`, if a receipt
/// already opened one; reusing it skips another read of the whole schema.
pub(crate) fn retained_ledger(path: &Path) -> Option<SqliteStore> {
    current_context()
        .filter(|context| context.ledger_path == path)
        .and_then(|context| context.ledger.get().cloned())
}

/// This Process's retained ledger connection, opening one only when no receipt
/// has yet; every open revalidates the schema.
pub(crate) fn process_ledger() -> Result<SqliteStore, crate::store::StoreError> {
    match current_context().and_then(|context| context.ledger.get().cloned()) {
        Some(store) => Ok(store),
        None => open_ledger(),
    }
}

#[cfg(not(test))]
fn ledger_db_path() -> Result<PathBuf, crate::store::StoreError> {
    crate::store::database_path_from_env()
        .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
}

/// Unit tests never resolve the ledger from process-global storage variables.
/// A test can opt into its own path through `TestLedgerGuard`; unguarded tests
/// share a process-local temporary ledger rather than touching a machine store.
#[cfg(test)]
fn ledger_db_path() -> Result<PathBuf, crate::store::StoreError> {
    if let Some(path) = TEST_LEDGER_DB_PATH.with(|path| path.borrow().clone()) {
        return Ok(path);
    }
    static TEST_HOME: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    Ok(TEST_HOME
        .get_or_init(|| tempfile::TempDir::new().expect("test ledger home"))
        .path()
        .join("loopflow.db"))
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
        None,
        receipt_wait(),
    )
}

fn create_process_context(
    repo_root: &Path,
    fields: &LfEventFields,
    ledger_path: PathBuf,
    ledger: Option<SqliteStore>,
    receipt_wait: Duration,
) -> Result<Option<ProcessContext>, std::io::Error> {
    let early = ledger.is_some();
    let same_store = ledger_db_path()
        .is_ok_and(|path| crate::store::same_database_file(&path, &ledger_path).unwrap_or(false));
    let main_repo = (!early).then(|| main_repo_root(repo_root).ok()).flatten();
    let wave_name = if early {
        None
    } else {
        let attribution = crate::work::wave::context::process_attribution(main_repo.as_deref());
        if let Some(failure) = attribution.failure.as_deref() {
            debug!(
                error = failure,
                "ambient wave identity failed validation; Process has no wave"
            );
        }
        attribution.wave
    };

    let agent_caller = same_store
        .then(|| std::env::var_os(AGENT_CALLER_ENV))
        .flatten()
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
    let agent_parent = agent_caller.as_ref().and_then(|caller| {
        match ledger
            .clone()
            .map_or_else(open_ledger, Ok)
            .and_then(|store| store.agent_parent(caller))
        {
            Ok(parent) => parent,
            Err(error) => {
                warn!(%error, "agent caller could not be resolved");
                None
            }
        }
    });
    let inherited_trace = if agent_caller.is_some() {
        agent_parent
            .as_ref()
            .and_then(|(_, trace)| TraceId::parse(trace).ok())
    } else {
        same_store.then(|| configured_trace_id(repo_root)).flatten()
    };
    let (trace_id, minted_trace_id) = match inherited_trace {
        Some(trace_id) => (trace_id, false),
        None => {
            // Mint and export the trace id so prompt logs and child processes
            // carry the same identity as the ledger rows. The export is
            // removed when the Process ends (see try_emit).
            let trace_id = TraceId::default();
            std::env::set_var(LF_TRACE_ID_ENV, trace_id.as_str());
            (trace_id, true)
        }
    };

    // A parent process id only means "my parent within this trace, recorded in
    // this ledger." A fresh trace id makes a lingering LF_PROCESS_ID belong to
    // the old trace; and `ledger_insert` is best-effort, so a parent whose
    // write never landed exported its identity anyway. Both spell a parent
    // that resolves to nothing. Drop it so the violation is unspellable at
    // write time; the trace id stays, so the trace still groups. A legitimate
    // parent records its own start row before it can spawn anything.
    let parent_lf_process_id = if agent_caller.is_some() {
        agent_parent.map(|(parent, _)| parent)
    } else {
        (!minted_trace_id)
            .then(|| {
                std::env::var(LF_PROCESS_ID_ENV)
                    .ok()
                    .and_then(|value| LfProcessId::parse(&value).ok())
            })
            .flatten()
            .filter(|parent| parent_is_recorded(parent, ledger.as_ref()))
    };
    std::env::set_var(LF_TRACE_ID_ENV, trace_id.as_str());
    let lf_process_id = LfProcessId::default();
    std::env::set_var(LF_PROCESS_ID_ENV, lf_process_id.as_str());

    // Write the file journal wherever we can. Fall back to ledger-only when
    // the journal can't be
    // git-excluded (e.g. not a git repo).
    let trace_dir = if early {
        None
    } else {
        match crate::repo::discover_repo_root(repo_root)
            .map_err(std::io::Error::other)
            .and_then(|root| {
                root.ok_or_else(|| std::io::Error::other("no repository for file journal"))
            })
            .and_then(|root| {
                ensure_journal_ignored(&root)?;
                let mut dir = traces_root(&root).join(trace_id.as_str());
                // Only an inherited Trace from before canonical storage can retain
                // another directory name; a minted or already-present one cannot.
                if !minted_trace_id && !dir.is_dir() {
                    let retained = ledger
                        .clone()
                        .map_or_else(open_ledger, Ok)
                        .and_then(|store| store.journal_trace_locator(&trace_id))
                        .map_err(std::io::Error::other)?;
                    if let Some(retained) = retained {
                        dir = traces_root(&root).join(retained);
                    }
                }
                fs::create_dir_all(&dir)?;
                Ok(dir)
            }) {
            Ok(dir) => Some(dir),
            Err(err) => {
                debug!(
                    error = %err,
                    repo = %repo_root.display(),
                    "file journal unavailable; recording to ledger only"
                );
                None
            }
        }
    };

    let repo = main_repo
        .as_deref()
        .unwrap_or(repo_root)
        .display()
        .to_string();

    let process_started_at = process_started_at(std::process::id()).unwrap_or_else(|error| {
        debug!(%error, "Process evidence unavailable; recording command history only");
        None
    });
    let context = ProcessContext {
        trace_id,
        lf_process_id,
        parent_lf_process_id,
        agent_caller,
        started_at: PROCESS_STARTED_AT
            .get()
            .copied()
            .unwrap_or_else(|| OffsetDateTime::now_utc().unix_timestamp()),
        process_started_at,
        cwd: repo_root.to_path_buf(),
        ledger_path,
        ledger: Arc::new(ledger.map(OnceLock::from).unwrap_or_default()),
        receipt_wait,
        command: fields
            .command
            .as_ref()
            .and_then(|argv| serde_json::to_string(argv).ok()),
        trace_dir,
        repo: (!early).then_some(repo),
        wave: wave_name.clone(),
        finished: Arc::new(AtomicBool::new(false)),
        minted_trace_id,
        receipts: Arc::default(),
    };
    set_context(context.clone());
    let interrupted = context.clone();
    crate::agent::register_interrupt_cleanup(move || record_process_interruption(&interrupted));
    // Never write process-control receipts into a different inherited Machine.
    if same_store {
        if let Err(error) = write_process_receipt(&context) {
            debug!(error = %error, lf_process_id = %context.lf_process_id, "live Process receipt unavailable");
        }
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

/// Whether the ledger holds a row for an inherited parent.
///
/// Read-only on purpose: this runs at the start of every nested `lf`, and
/// asking who my parent was must not migrate, back up, or take the exclusive
/// lock a full open does.
///
/// A ledger that isn't there records nothing, so a missing file answers
/// `false`. Any other read failure answers `true` — never disown a real parent
/// over a locked store; this process's own row is about to fail the same way,
/// so there is no ghost to prevent.
fn parent_is_recorded(parent: &LfProcessId, ledger: Option<&SqliteStore>) -> bool {
    let path = match ledger_db_path() {
        Ok(path) if path.exists() => path,
        Ok(_) => return false,
        Err(err) => {
            debug!(error = %err, "no ledger path; keeping the inherited parent process id");
            return true;
        }
    };
    match ledger
        .cloned()
        .map_or_else(|| SqliteStore::open_processes_read_only(&path), Ok)
        .and_then(|store| store.process_is_recorded(parent.as_str()))
    {
        Ok(recorded) => recorded,
        Err(err) => {
            debug!(
                error = %err,
                parent = parent.as_str(),
                "ledger unreadable; keeping the inherited parent process id"
            );
            true
        }
    }
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
    if is_cli_process() {
        return PROCESS_CONTEXT
            .lock()
            .expect("process context mutex poisoned")
            .clone();
    }
    THREAD_CONTEXT.with(|cell| cell.borrow().clone())
}

/// Caller provenance captured at process entry, before the environment is consumed.
pub fn agent_caller() -> Option<AgentCaller> {
    current_context().and_then(|context| context.agent_caller)
}

/// A nested command leaves checkpoint composition to its caller.
pub fn has_caller() -> bool {
    current_context().is_some_and(|context| {
        context.agent_caller.is_some() || context.parent_lf_process_id.is_some()
    })
}

pub(crate) fn current_lf_process_id() -> Option<LfProcessId> {
    current_context().map(|context| context.lf_process_id)
}

pub(crate) fn process_identity_evidence(pid: u32, started_at: i64) -> ProcessIdentityEvidence {
    match OsProcess::read(pid) {
        Ok(Some(process)) => process.evidence(started_at),
        Ok(None) => ProcessIdentityEvidence::Dead,
        Err(_) => ProcessIdentityEvidence::Unknown,
    }
}

pub(crate) fn process_evidence(
    store: &SqliteStore,
    process: &LfProcessId,
) -> ProcessIdentityEvidence {
    let Ok(Some(record)) = store.process(process) else {
        return ProcessIdentityEvidence::Unknown;
    };
    recorded_process_evidence(
        &record,
        &recorded_process_receipt(store, &record),
        process_identity_evidence,
    )
}

/// The recorded Process's own receipt, read by its locator without listing others.
pub(crate) fn recorded_process_receipt(
    store: &SqliteStore,
    record: &crate::process::LfProcess,
) -> Result<Option<ProcessReceipt>, std::io::Error> {
    if record.kind == crate::process::ProcessKind::Agent {
        return Ok(None);
    }
    match store
        .process_receipt_path(&record.id)
        .map_err(std::io::Error::other)?
    {
        Some(path) => read_process_receipt(&path),
        None => Ok(None),
    }
}

pub(crate) fn process_receipt_path(lf_home: &Path, locator: &str) -> PathBuf {
    // Retained receipt basenames are opaque. A case-normalized Process keeps
    // reading the original file, including on case-sensitive filesystems.
    lf_home
        .join(PROCESS_RECEIPT_ROOT)
        .join(format!("{locator}.json"))
}

pub(crate) fn read_process_receipt(path: &Path) -> Result<Option<ProcessReceipt>, std::io::Error> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let receipt: ProcessReceipt = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
    if receipt.schema_version != 1 {
        return Err(std::io::Error::other("unsupported Process receipt schema"));
    }
    Ok(Some(receipt))
}

/// Completion is final. Otherwise only the recorded OS identity proves liveness;
/// unknown evidence never ages into permission to replace or clean up a process.
/// A restart since `began_at` (the Process record's start) still proves exit.
pub(crate) fn agent_process_evidence(
    completed_at: Option<i64>,
    pid: Option<u32>,
    started_at: Option<i64>,
    began_at: Option<i64>,
    mut observe: impl FnMut(u32, i64) -> ProcessIdentityEvidence,
) -> ProcessIdentityEvidence {
    if completed_at.is_some() {
        ProcessIdentityEvidence::Dead
    } else if let Some((pid, start)) = pid.zip(started_at) {
        observe(pid, start)
    } else if began_at.is_some_and(began_before_boot) {
        ProcessIdentityEvidence::Dead
    } else {
        ProcessIdentityEvidence::Unknown
    }
}

/// One identity rule for control readers and sampled activity. An unreadable
/// receipt is not an absent one; neither permits invented OS identity.
pub(crate) fn recorded_process_evidence(
    record: &crate::process::LfProcess,
    receipt: &Result<Option<ProcessReceipt>, std::io::Error>,
    mut observe: impl FnMut(u32, i64) -> ProcessIdentityEvidence,
) -> ProcessIdentityEvidence {
    if record.kind == crate::process::ProcessKind::Agent {
        return agent_process_evidence(
            record.completed_at,
            record.pid,
            record.os_started_at,
            Some(record.started_at),
            observe,
        );
    }
    let Ok(receipt) = receipt else {
        return ProcessIdentityEvidence::Unknown;
    };
    match receipt {
        Some(receipt) if receipt.matches_process(record.id.as_str(), record.trace_id.as_str()) => {
            return observe(receipt.pid, receipt.started_at);
        }
        // Without a receipt, an unfinished row's own OS identity still tells a reused PID.
        None if record.completed_at.is_none() => {
            if let Some((pid, started_at)) = record.pid.zip(record.os_started_at) {
                return observe(pid, started_at);
            }
        }
        _ => {}
    }
    // Historical Processes can lack identity evidence. A restart still proves exit.
    if record.completed_at.is_some() || began_before_boot(record.started_at) {
        ProcessIdentityEvidence::Dead
    } else {
        ProcessIdentityEvidence::Unknown
    }
}

#[cfg(test)]
thread_local! {
    static TEST_MACHINE_BOOTED_AT: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
}

/// Pretend this machine booted at `at` for the rest of the test thread.
#[cfg(test)]
pub(crate) fn set_test_machine_booted_at(at: Option<i64>) {
    TEST_MACHINE_BOOTED_AT.with(|cell| cell.set(at));
}

/// When this machine last booted. No local process survives that boundary, so
/// work that began earlier and never settled has exited.
pub(crate) fn machine_booted_at() -> Option<i64> {
    #[cfg(test)]
    if let Some(at) = TEST_MACHINE_BOOTED_AT.with(std::cell::Cell::get) {
        return Some(at);
    }
    static BOOTED_AT: OnceLock<Option<i64>> = OnceLock::new();
    *BOOTED_AT.get_or_init(read_machine_booted_at)
}

pub(crate) fn began_before_boot(started_at: i64) -> bool {
    machine_booted_at().is_some_and(|booted_at| started_at < booted_at)
}

fn read_machine_booted_at() -> Option<i64> {
    if let Ok(stat) = fs::read_to_string("/proc/stat") {
        return stat
            .lines()
            .find_map(|line| line.strip_prefix("btime "))
            .and_then(|value| value.trim().parse().ok());
    }
    let output = Command::new("sysctl")
        .args(["-n", "kern.boottime"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    parse_sysctl_boottime(&String::from_utf8_lossy(&output.stdout))
}

/// `{ sec = 1791140551, usec = 157377 } Sun Oct  4 12:02:31 2026`
fn parse_sysctl_boottime(value: &str) -> Option<i64> {
    let seconds = value.split_once("sec = ")?.1;
    seconds.split([',', ' ']).next()?.parse().ok()
}

pub(crate) fn process_started_at(pid: u32) -> Result<Option<i64>, std::io::Error> {
    Ok(OsProcess::read(pid)?.map(|process| process.started_at))
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

pub(crate) fn read_process_receipts_at(
    lf_home: &Path,
) -> Result<Vec<ProcessReceipt>, std::io::Error> {
    Ok(read_process_receipt_files_at(lf_home)?
        .into_iter()
        .map(|(_, receipt)| receipt)
        .collect())
}

fn read_process_receipt_files_at(
    lf_home: &Path,
) -> Result<Vec<(PathBuf, ProcessReceipt)>, std::io::Error> {
    let root = lf_home.join(PROCESS_RECEIPT_ROOT);
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut receipts = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        if let Ok(Some(receipt)) = read_process_receipt(&path) {
            receipts.push((path, receipt));
        }
    }
    Ok(receipts)
}

pub(crate) fn prune_process_receipts_at(
    lf_home: &Path,
    pids: &[u32],
) -> Result<u32, std::io::Error> {
    if pids.is_empty() {
        return Ok(0);
    }
    let store = SqliteStore::open_processes_read_only(&lf_home.join("loopflow.db"))
        .map_err(std::io::Error::other)?;
    let mut removed = 0;
    for (path, receipt) in read_process_receipt_files_at(lf_home)? {
        if !pids.contains(&receipt.pid)
            || receipt.process_evidence() != ProcessIdentityEvidence::Dead
        {
            continue;
        }
        let retained = match path.file_stem().and_then(|stem| stem.to_str()) {
            Some(locator) => store
                .process_for_receipt(locator)
                .map_err(std::io::Error::other)?,
            None => None,
        };
        // Earlier receipts were named by PID; their recorded Process owns them.
        let record = match retained {
            Some(record) => Some(record),
            None => match LfProcessId::parse(&receipt.lf_process_id) {
                Ok(id) => store.process(&id).map_err(std::io::Error::other)?,
                Err(_) => None,
            },
        };
        // An unfinished row's receipt is the only evidence of its exit. Only a
        // Process writes its own row, so a dead one without a row never gets one.
        let removable = record.is_none_or(|record| {
            receipt.matches_process(record.id.as_str(), record.trace_id.as_str())
                && record.completed_at.is_some()
        });
        if !removable {
            continue;
        }
        match fs::remove_file(path) {
            Ok(()) => removed += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(removed)
}

fn write_process_receipt(context: &ProcessContext) -> Result<(), std::io::Error> {
    let Some(started_at) = context.process_started_at else {
        return Ok(());
    };
    let home = context
        .ledger_path
        .parent()
        .ok_or_else(|| std::io::Error::other("Process ledger has no parent directory"))?;
    let root = home.join(PROCESS_RECEIPT_ROOT);
    fs::create_dir_all(&root)?;
    let pid = std::process::id();
    let receipt = ProcessReceipt {
        schema_version: 1,
        trace_id: context.trace_id.to_string(),
        lf_process_id: context.lf_process_id.to_string(),
        pid,
        started_at,
    };
    let bytes = serde_json::to_vec(&receipt).map_err(std::io::Error::other)?;
    let path = process_receipt_path(home, context.lf_process_id.as_str());
    let temporary = root.join(format!(".{}.json.tmp", context.lf_process_id));
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)
}

fn remove_process_receipt(context: &ProcessContext) {
    let Some(home) = context.ledger_path.parent() else {
        return;
    };
    let path = process_receipt_path(home, context.lf_process_id.as_str());
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            debug!(error = %error, "failed to remove live Process receipt");
        }
    }
}

fn ensure_journal_ignored(repo_root: &Path) -> Result<(), std::io::Error> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["rev-parse", "--git-path", "info/exclude"])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "git rev-parse --git-path info/exclude failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    // `--git-path` answers relative to the repo when run at its root (main
    // repos) and absolute for linked worktrees — absolutize before writing.
    let mut exclude_path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if exclude_path.is_relative() {
        exclude_path = repo_root.join(exclude_path);
    }
    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    if existing
        .lines()
        .any(|line| line.trim() == JOURNAL_EXCLUDE_ENTRY)
    {
        return Ok(());
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(JOURNAL_EXCLUDE_ENTRY);
    updated.push('\n');
    fs::write(exclude_path, updated)
}

#[cfg(test)]
mod tests {
    use super::{
        emit, events_path, read_events, traces_root, LfEvent, LfEventFields, LfEventType, LfNode,
        ProcessIdentityEvidence, TestLedgerGuard,
    };
    use crate::git::is_clean;
    use crate::id::{LfProcessId, TraceId};
    use loopflow_test_support::{RunningParent, TestRepo};
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    #[test]
    fn agent_without_os_identity_is_dead_only_after_a_restart() {
        let unobserved = |_: u32, _: i64| -> ProcessIdentityEvidence { unreachable!() };
        super::set_test_machine_booted_at(Some(1_000));
        let before = super::agent_process_evidence(None, None, None, Some(999), unobserved);
        let since = super::agent_process_evidence(None, None, None, Some(1_001), unobserved);
        let unrecorded = super::agent_process_evidence(None, None, None, None, unobserved);
        super::set_test_machine_booted_at(None);
        assert_eq!(before, ProcessIdentityEvidence::Dead);
        assert_eq!(since, ProcessIdentityEvidence::Unknown);
        assert_eq!(unrecorded, ProcessIdentityEvidence::Unknown);
    }

    #[test]
    fn boot_time_parses_and_bounds_what_can_still_run() {
        assert_eq!(
            super::parse_sysctl_boottime(
                "{ sec = 1791140551, usec = 157377 } Sun Oct  4 12:02:31 2026"
            ),
            Some(1_791_140_551)
        );
        assert_eq!(super::parse_sysctl_boottime("unavailable"), None);
        // This test began after the machine that runs it booted.
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        assert!(super::machine_booted_at().is_some_and(|booted_at| booted_at <= now));
        assert!(!super::began_before_boot(now));
        assert!(super::began_before_boot(1));
    }

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

    /// Export a [`RunningParent`] as the parent of the next Process.
    fn adopt_by_running_parent(guard: &TestLedgerGuard) {
        super::open_ledger().unwrap();
        let parent = RunningParent::insert(&guard.home().join("loopflow.db"));
        std::env::set_var(super::LF_PROCESS_ID_ENV, &parent.id);
        std::env::set_var(super::LF_TRACE_ID_ENV, &parent.trace);
    }

    #[test]
    fn canonical_ids_journal_appends_to_retained_trace_directory() {
        let guard = journal_test_guard();
        let repo = TestRepo::new();
        super::open_ledger().unwrap();
        let conn = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        let trace = "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC";
        conn.execute(
            "INSERT INTO processes(id,trace_id,journal_trace_locator,started_at,completed_at) VALUES(?1,?2,?3,1,2)",
            rusqlite::params![LfProcessId::new(), trace.to_ascii_lowercase(), trace],
        )
        .unwrap();
        let retained = traces_root(repo.path()).join(trace);
        std::fs::create_dir_all(&retained).unwrap();
        std::fs::write(retained.join("retained-evidence"), b"unchanged").unwrap();
        std::env::set_var(super::LF_TRACE_ID_ENV, trace.to_ascii_lowercase());
        super::with_runtime(repo.path(), &["lf".into(), "fixture".into()], || {
            let process = super::current_lf_process_id().unwrap();
            let store = super::open_ledger().unwrap();
            assert_eq!(
                super::process_evidence(&store, &process),
                ProcessIdentityEvidence::Live
            );
            Ok(())
        })
        .unwrap();
        std::env::remove_var(super::LF_TRACE_ID_ENV);
        let directories: Vec<_> = std::fs::read_dir(traces_root(repo.path()))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(directories, [std::ffi::OsString::from(trace)]);
        assert_eq!(
            std::fs::read(retained.join("retained-evidence")).unwrap(),
            b"unchanged"
        );
        let events = read_events(&retained).unwrap();
        assert_eq!(events.len(), 2);
        assert!(events
            .iter()
            .all(|event| event.trace_id.as_str() == trace.to_ascii_lowercase()));
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

        // The parent emits, which is what puts its row in the ledger and its
        // identity in the environment. A child only inherits a parent the
        // ledger holds, so the write is the thing under test, not a fixture.
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
    fn an_inherited_parent_the_ledger_never_recorded_is_dropped() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let fields = started_fields(
            &["lf".to_string(), "status".to_string()],
            repo.path(),
            "main",
        );

        // A real Process first, so the ledger exists and holds rows. Without it the
        // drop proves nothing: an absent database answers "not recorded" for
        // every id, and this passes with the lookup hardwired to `true`.
        super::emit(
            repo.path(),
            LfNode::Process,
            LfEventType::Started,
            fields.clone(),
        );
        let recorded = super::current_context().expect("recorded Process");
        super::clear_context();

        // A parent that exported its identity but never reached the ledger.
        let ghost = LfProcessId::new();
        std::env::set_var(super::LF_TRACE_ID_ENV, recorded.trace_id.as_str());
        std::env::set_var(super::LF_PROCESS_ID_ENV, ghost.as_str());

        let context = super::ensure_process_context(repo.path(), &fields)
            .expect("Process context")
            .expect("context");

        assert!(
            super::parent_is_recorded(&recorded.lf_process_id, None),
            "the ledger must really hold a parent, or the assertion below \
             passes for the wrong reason"
        );

        assert_eq!(
            context.parent_lf_process_id, None,
            "a parent the ledger never recorded is a ghost, not lineage"
        );
        assert!(
            !context.minted_trace_id,
            "the trace still groups; only the false pointer goes"
        );

        super::clear_context();
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        std::env::remove_var(super::LF_TRACE_ID_ENV);
    }

    #[test]
    fn a_detached_body_keeps_its_trace_after_its_launcher_is_forgotten() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let fields = started_fields(&["lf".to_string(), "task".to_string()], repo.path(), "main");

        // A detached body inherits LF_TRACE_ID/LF_PROCESS_ID from a launcher that
        // has already exited. Nothing referenced the launcher, so its exit forgot
        // it: the body keeps the trace and drops the parent edge.
        super::emit(
            repo.path(),
            LfNode::Process,
            LfEventType::Started,
            fields.clone(),
        );
        let launcher = super::current_context().expect("launcher");
        super::emit(
            repo.path(),
            LfNode::Process,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        // The body carries what the launcher handed it, not what the launcher
        // left behind: a terminal Process clears LF_PROCESS_ID from the env.
        std::env::set_var(super::LF_TRACE_ID_ENV, launcher.trace_id.as_str());
        std::env::set_var(super::LF_PROCESS_ID_ENV, launcher.lf_process_id.as_str());
        super::clear_context();

        let body = super::ensure_process_context(repo.path(), &fields)
            .expect("body context")
            .expect("body");

        assert_eq!(body.parent_lf_process_id, None);
        assert_eq!(body.trace_id, launcher.trace_id);

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
    fn process_lifecycle_publishes_and_removes_exact_process_ownership() {
        let guard = journal_test_guard();
        let repo = TestRepo::new();
        let worktree = repo.create_named_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Process,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        let receipts = super::read_process_receipts_at(guard.home()).expect("read live receipt");
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].pid, std::process::id());
        assert_eq!(
            receipts[0].lf_process_id,
            std::env::var(super::LF_PROCESS_ID_ENV).expect("current Process id")
        );

        emit(
            &worktree,
            LfNode::Process,
            LfEventType::Completed,
            LfEventFields::default(),
        );
        assert!(super::read_process_receipts_at(guard.home())
            .expect("read terminal receipt state")
            .is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn attachment_identity_needs_no_receipt_directory_listing() {
        use std::os::unix::fs::PermissionsExt;

        let guard = journal_test_guard();
        let repo = TestRepo::new();
        emit(
            repo.path(),
            LfNode::Process,
            LfEventType::Started,
            started_fields(&["lf".into()], repo.path(), "runtime"),
        );
        let context = super::current_context().unwrap();
        let store = super::open_ledger().unwrap();
        let root = guard.home().join(super::PROCESS_RECEIPT_ROOT);
        let permissions = std::fs::metadata(&root).unwrap().permissions();
        // Traversal permits the ID-keyed read; enumerating unrelated receipts
        // is unavailable. This bounds the work without a wall-time assertion.
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o111)).unwrap();
        let evidence = super::process_evidence(&store, &context.lf_process_id);
        std::fs::set_permissions(&root, permissions).unwrap();
        assert_eq!(evidence, ProcessIdentityEvidence::Live);
    }

    #[test]
    fn attachment_receipt_failures_never_invent_process_identity() {
        let guard = journal_test_guard();
        let repo = TestRepo::new();
        // Its finished row stays for the final Dead answer.
        adopt_by_running_parent(&guard);
        emit(
            repo.path(),
            LfNode::Process,
            LfEventType::Started,
            started_fields(&["lf".into()], repo.path(), "runtime"),
        );
        let context = super::current_context().unwrap();
        let store = super::open_ledger().unwrap();
        let path = guard
            .home()
            .join(super::PROCESS_RECEIPT_ROOT)
            .join(format!("{}.json", context.lf_process_id));
        let original = std::fs::read(&path).unwrap();
        let receipt: super::ProcessReceipt = serde_json::from_slice(&original).unwrap();
        for changed in [
            super::ProcessReceipt {
                schema_version: 2,
                ..receipt.clone()
            },
            super::ProcessReceipt {
                trace_id: "another-trace".into(),
                ..receipt.clone()
            },
            super::ProcessReceipt {
                lf_process_id: LfProcessId::new().to_string(),
                ..receipt
            },
        ] {
            std::fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
            assert_eq!(
                super::process_evidence(&store, &context.lf_process_id),
                ProcessIdentityEvidence::Unknown
            );
        }
        std::fs::write(&path, b"partial json").unwrap();
        assert_eq!(
            super::process_evidence(&store, &context.lf_process_id),
            ProcessIdentityEvidence::Unknown
        );
        // Without a receipt, the Process row's own OS identity still answers.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            super::process_evidence(&store, &context.lf_process_id),
            ProcessIdentityEvidence::Live
        );
        std::fs::create_dir(&path).unwrap();
        assert_eq!(
            super::process_evidence(&store, &context.lf_process_id),
            ProcessIdentityEvidence::Unknown
        );
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, original).unwrap();
        emit(
            repo.path(),
            LfNode::Process,
            LfEventType::Completed,
            LfEventFields::default(),
        );
        assert_eq!(
            super::process_evidence(&store, &context.lf_process_id),
            ProcessIdentityEvidence::Dead
        );
    }

    #[test]
    fn prune_removes_dead_receipts_unless_an_unfinished_row_needs_them() {
        let guard = journal_test_guard();
        super::open_ledger().unwrap();
        let sql = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        let pid = std::process::id();
        let born = super::process_started_at(pid).unwrap().unwrap();
        let root = guard.home().join(super::PROCESS_RECEIPT_ROOT);
        std::fs::create_dir_all(&root).unwrap();
        let receipt = |started_at: i64, row: Option<Option<i64>>| {
            let id = LfProcessId::new();
            if let Some(completed_at) = row {
                sql.execute(
                    "INSERT INTO processes(id,trace_id,started_at,completed_at) VALUES(?1,?1,1,?2)",
                    rusqlite::params![id, completed_at],
                )
                .unwrap();
            }
            let path = root.join(format!("{id}.json"));
            let receipt = super::ProcessReceipt {
                schema_version: 1,
                trace_id: id.to_string(),
                lf_process_id: id.to_string(),
                pid,
                started_at,
            };
            std::fs::write(&path, serde_json::to_vec(&receipt).unwrap()).unwrap();
            path
        };
        // A different birth time under this PID is affirmative death.
        let completed = receipt(born - 60, Some(Some(2)));
        let unrecorded = receipt(born - 60, None);
        let unfinished = receipt(born - 60, Some(None));
        let live = receipt(born, None);

        assert_eq!(
            super::prune_process_receipts_at(guard.home(), &[pid]).unwrap(),
            2
        );
        assert!(!completed.exists());
        assert!(!unrecorded.exists());
        assert!(unfinished.exists());
        assert!(live.exists());
    }

    #[test]
    fn unfinished_process_retains_identity_across_terminal_failure_interrupt_and_pid_reuse() {
        let guard = journal_test_guard();
        let repo = TestRepo::new();
        for interrupt in [false, true] {
            emit(
                repo.path(),
                LfNode::Process,
                LfEventType::Started,
                started_fields(&["lf".into(), "task".into()], repo.path(), "runtime"),
            );
            let context = super::current_context().unwrap();
            let store = super::open_ledger().unwrap();
            let before = store.process(&context.lf_process_id).unwrap().unwrap();
            assert!(before.completed_at.is_none());
            assert_eq!(before.pid, Some(std::process::id()));
            let foreign = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
            foreign.execute_batch("BEGIN IMMEDIATE").unwrap();
            super::TEST_RECEIPT_WAIT.with(|wait| wait.set(std::time::Duration::ZERO));
            if interrupt {
                super::record_process_interruption(&context);
                super::clear_context();
            } else {
                emit(
                    repo.path(),
                    LfNode::Process,
                    LfEventType::Completed,
                    LfEventFields::default(),
                );
                // Cleanup following the failed ordinary finish must not erase identity.
                super::record_process_interruption(&context);
            }
            foreign.execute_batch("ROLLBACK").unwrap();
            assert_eq!(
                super::process_evidence(&store, &context.lf_process_id),
                ProcessIdentityEvidence::Live
            );

            // Another Process with the same PID must neither replace nor remove this receipt.
            let mut replacement = context.clone();
            replacement.lf_process_id = LfProcessId::new();
            super::write_process_receipt(&replacement).unwrap();
            super::remove_process_receipt(&replacement);
            assert_eq!(
                super::prune_process_receipts_at(guard.home(), &[std::process::id()]).unwrap(),
                0
            );
            assert_eq!(
                super::process_evidence(&store, &context.lf_process_id),
                ProcessIdentityEvidence::Live
            );

            // macOS reads identity natively; other hosts need ps on PATH.
            let previous_path = std::env::var_os("PATH");
            std::env::set_var("PATH", guard.home().join("no-programs"));
            let without_path = super::process_evidence(&store, &context.lf_process_id);
            let pruned = super::prune_process_receipts_at(guard.home(), &[std::process::id()]);
            match previous_path {
                Some(path) => std::env::set_var("PATH", path),
                None => std::env::remove_var("PATH"),
            }
            assert_eq!(
                without_path,
                if cfg!(target_os = "macos") {
                    ProcessIdentityEvidence::Live
                } else {
                    ProcessIdentityEvidence::Unknown
                }
            );
            assert_eq!(pruned.unwrap(), 0);

            // Model a reused PID: the retained birth belongs to an earlier process.
            let mut dead = context.clone();
            dead.process_started_at = Some(context.process_started_at.unwrap() - 60);
            super::write_process_receipt(&dead).unwrap();
            assert_eq!(
                super::process_evidence(&store, &context.lf_process_id),
                ProcessIdentityEvidence::Dead
            );
            assert_eq!(
                super::prune_process_receipts_at(guard.home(), &[std::process::id()]).unwrap(),
                0
            );
            assert_eq!(
                store.process(&context.lf_process_id).unwrap().unwrap(),
                before
            );
        }
    }

    /// Run one Process while another connection holds SQLite's write lock, through
    /// the finish receipt unless it is released first.
    fn process_under_foreign_write_lock(
        wait: std::time::Duration,
        release_before_finish: bool,
    ) -> (super::ReceiptCost, bool) {
        let guard = journal_test_guard();
        let repo = TestRepo::new();
        let command = vec!["lf".to_string(), "wt".to_string(), "list".to_string()];
        let run = |finish: &dyn Fn()| {
            emit(
                repo.path(),
                LfNode::Process,
                LfEventType::Started,
                started_fields(&command, repo.path(), "main"),
            );
            let context = super::current_context().expect("Process context");
            finish();
            emit(
                repo.path(),
                LfNode::Process,
                LfEventType::Completed,
                LfEventFields::default(),
            );
            context
        };
        // An initialized Machine, as every real listing has.
        run(&|| {});

        // A running parent keeps the finished row a landed receipt writes.
        adopt_by_running_parent(&guard);
        let ledger = guard.home().join("loopflow.db");
        let foreign = rusqlite::Connection::open(&ledger).unwrap();
        foreign.execute_batch("BEGIN IMMEDIATE").unwrap();
        super::TEST_RECEIPT_WAIT.with(|current| current.set(wait));
        let context = run(&|| {
            if release_before_finish {
                foreign.execute_batch("ROLLBACK").unwrap();
            }
        });
        if !release_before_finish {
            foreign.execute_batch("ROLLBACK").unwrap();
        }

        let cost = *context.receipts.lock().unwrap();
        let recorded = super::open_ledger()
            .unwrap()
            .process_is_recorded(context.lf_process_id.as_str())
            .unwrap();
        (cost, recorded)
    }

    #[test]
    fn a_held_write_lock_delays_an_process_by_one_receipt_wait() {
        let wait = std::time::Duration::from_millis(1500);

        let (cost, recorded) = process_under_foreign_write_lock(wait, false);

        assert!(cost.waited >= wait, "start receipt waited: {cost:?}");
        assert!(
            cost.waited < wait * 2,
            "finish receipt waited again: {cost:?}"
        );
        assert_eq!(cost.unrecorded, 2);
        assert!(!recorded);
    }

    #[test]
    fn a_finish_receipt_lands_whole_once_the_write_lock_clears() {
        let (cost, recorded) =
            process_under_foreign_write_lock(std::time::Duration::from_millis(300), true);

        assert_eq!(cost.unrecorded, 1);
        assert!(recorded);
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
