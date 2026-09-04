use std::cell::RefCell;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::{debug, warn};

use crate::engine::worktrees::main_repo_root;
use crate::id::{ExecId, TraceId};
use crate::store::sqlite::SqliteStore;
use crate::store::RunEventRow;

const JOURNAL_ROOT: &str = ".lf/journal/runs";
const JOURNAL_EXCLUDE_ENTRY: &str = ".lf/journal/";
const EXEC_PROCESS_ROOT: &str = "runtime/exec-processes";
pub const LF_TRACE_ID_ENV: &str = "LF_TRACE_ID";
pub const LF_PROCESS_ID_ENV: &str = "LF_PROCESS_ID";

/// Serializes tests that mutate process-global store or run identity variables.
/// Every test in the crate that touches these variables must hold this lock.
#[cfg(test)]
pub(crate) fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
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
    previous_db_path: Option<std::ffi::OsString>,
    previous_control_home: Option<std::ffi::OsString>,
    previous_control_db_path: Option<std::ffi::OsString>,
    previous_test_path: Option<PathBuf>,
    home: tempfile::TempDir,
}

#[cfg(test)]
impl TestLedgerGuard {
    pub(crate) fn new() -> Self {
        let lock = test_env_lock();
        let home = tempfile::TempDir::new().expect("test ledger home");
        let previous_lf_home = std::env::var_os("LF_HOME");
        let previous_db_path = std::env::var_os("LF_DB_PATH");
        let previous_control_home = std::env::var_os(crate::store::CONTROL_HOME_ENV);
        let previous_control_db_path = std::env::var_os(crate::store::CONTROL_DB_PATH_ENV);
        std::env::remove_var("LF_HOME");
        std::env::remove_var("LF_DB_PATH");
        std::env::remove_var(crate::store::CONTROL_HOME_ENV);
        std::env::remove_var(crate::store::CONTROL_DB_PATH_ENV);
        std::env::set_var("LF_HOME", home.path());
        let previous_test_path =
            TEST_LEDGER_DB_PATH.with(|path| path.replace(Some(home.path().join("loopflow.db"))));
        Self {
            _lock: lock,
            previous_lf_home,
            previous_db_path,
            previous_control_home,
            previous_control_db_path,
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

#[cfg(test)]
impl Drop for TestLedgerGuard {
    fn drop(&mut self) {
        TEST_LEDGER_DB_PATH.with(|path| *path.borrow_mut() = self.previous_test_path.take());
        match &self.previous_lf_home {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }
        match &self.previous_db_path {
            Some(value) => std::env::set_var("LF_DB_PATH", value),
            None => std::env::remove_var("LF_DB_PATH"),
        }
        match &self.previous_control_home {
            Some(value) => std::env::set_var(crate::store::CONTROL_HOME_ENV, value),
            None => std::env::remove_var(crate::store::CONTROL_HOME_ENV),
        }
        match &self.previous_control_db_path {
            Some(value) => std::env::set_var(crate::store::CONTROL_DB_PATH_ENV, value),
            None => std::env::remove_var(crate::store::CONTROL_DB_PATH_ENV),
        }
    }
}

thread_local! {
    static RUN_CONTEXT: RefCell<Option<RunContext>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone)]
struct RunContext {
    run_id: TraceId,
    process_id: ExecId,
    parent_process_id: Option<ExecId>,
    /// Serialized argv captured at run start so terminal rows name their work.
    command: Option<String>,
    /// File-journal directory. Written in any git checkout; None only when the
    /// journal can't be git-excluded. The SQLite ledger records every run.
    run_dir: Option<PathBuf>,
    repo: Option<String>,
    wave: Option<String>,
    seq: i64,
    /// True when this process minted the run id (vs inheriting LF_TRACE_ID);
    /// the export is removed again when the run ends.
    minted_run_id: bool,
}

/// Exact, process-local ownership evidence for a live Loopflow Exec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ExecProcessReceipt {
    pub schema_version: u32,
    pub trace_id: String,
    pub exec_id: String,
    pub pid: u32,
    pub started_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LfNode {
    Run,
    Flow,
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
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub index: Option<u32>,
    pub error: Option<String>,
    pub signal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LfEvent {
    pub run_id: TraceId,
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
    pub flow: Option<String>,
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

pub fn runs_root(worktree: &Path) -> PathBuf {
    worktree.join(JOURNAL_ROOT)
}

pub fn events_path(run_dir: &Path) -> PathBuf {
    run_dir.join("events.jsonl")
}

pub fn read_events(run_dir: &Path) -> Result<Vec<LfEvent>, std::io::Error> {
    let path = events_path(run_dir);
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
    let is_run_started = matches!((node, event), (LfNode::Run, LfEventType::Started));
    let maybe_context = if is_run_started {
        ensure_run_context(repo_root, &fields)?
    } else {
        current_context()
    };

    let Some(context) = maybe_context else {
        return Ok(());
    };

    let event = LfEvent {
        run_id: context.run_id.clone(),
        ts: OffsetDateTime::now_utc(),
        node,
        event,
        wave_name: fields.wave_name,
        worktree: fields.worktree,
        command: fields.command,
        flow: fields.flow,
        skill: fields.skill,
        index: fields.index,
        error: fields.error,
        signal: fields.signal,
    };

    if let Some(run_dir) = &context.run_dir {
        append_event(run_dir, &event)?;
    }

    let seq = next_seq();
    ledger_insert(&context, &event, seq, repo_root);

    if matches!(node, LfNode::Run)
        && matches!(
            event.event,
            LfEventType::Completed | LfEventType::Errored | LfEventType::Escalated
        )
    {
        remove_exec_process_receipt();
        if context.minted_run_id {
            std::env::remove_var(LF_TRACE_ID_ENV);
        }
        std::env::remove_var(LF_PROCESS_ID_ENV);
        clear_context();
    }

    Ok(())
}

/// Best-effort write into the machine-grain SQLite ledger. Never fails the
/// run: a locked or missing store degrades to a debug log line. Local-only —
/// the ledger never leaves the machine.
fn ledger_insert(context: &RunContext, event: &LfEvent, seq: i64, repo_root: &Path) {
    let row = RunEventRow {
        run_id: event.run_id.as_str().to_string(),
        process_id: context.process_id.as_str().to_string(),
        parent_process_id: context
            .parent_process_id
            .as_ref()
            .map(|id| id.as_str().to_string()),
        seq,
        ts: event.ts.unix_timestamp(),
        repo: context.repo.clone(),
        worktree: Some(repo_root.display().to_string()),
        wave: context.wave.clone(),
        node: node_name(event.node).to_string(),
        event: event_name(event.event).to_string(),
        command: event
            .command
            .as_ref()
            .and_then(|argv| serde_json::to_string(argv).ok())
            .or_else(|| context.command.clone()),
        flow: event.flow.clone(),
        skill: event.skill.clone(),
        step_index: event.index.map(i64::from),
        error: event.error.clone(),
    };

    match open_ledger() {
        Ok(store) => {
            if let Err(err) = store.insert_run_event(&row) {
                if first_ledger_failure() {
                    warn!(error = %err, run_id = %row.run_id, "ledger insert failed — this run is not being recorded");
                } else {
                    debug!(error = %err, run_id = %row.run_id, "ledger insert failed");
                }
            }
        }
        Err(err) => {
            if first_ledger_failure() {
                warn!(error = %err, "ledger unavailable — runs are not being recorded");
            } else {
                debug!(error = %err, "ledger unavailable");
            }
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
    SqliteStore::new(&ledger_db_path()?)
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

fn node_name(node: LfNode) -> &'static str {
    match node {
        LfNode::Run => "run",
        LfNode::Flow => "flow",
        LfNode::Skill => "skill",
    }
}

fn event_name(event: LfEventType) -> &'static str {
    match event {
        LfEventType::Started => "started",
        LfEventType::Completed => "completed",
        LfEventType::Errored => "errored",
        LfEventType::Escalated => "escalated",
    }
}

fn ensure_run_context(
    repo_root: &Path,
    fields: &LfEventFields,
) -> Result<Option<RunContext>, std::io::Error> {
    if let Some(context) = current_context() {
        return Ok(Some(context));
    }

    let main_repo = main_repo_root(repo_root).ok();
    let attribution = crate::work::wave::context::run_attribution(main_repo.as_deref());
    let wave_name = attribution.wave;
    if let Some(failure) = attribution.failure.as_deref() {
        debug!(
            error = failure,
            repo = %repo_root.display(),
            "ambient wave identity failed validation; run attributed to no wave, \
             not inferred from the worktree"
        );
    }

    let (run_id, minted_run_id) = match configured_run_id(repo_root) {
        Some(run_id) => (run_id, false),
        None => {
            // Mint and export the run id so prompt logs and child processes
            // carry the same identity as the ledger rows. The export is
            // removed when the run ends (see try_emit).
            let run_id = TraceId::default();
            std::env::set_var(LF_TRACE_ID_ENV, run_id.as_str());
            (run_id, true)
        }
    };

    // A parent process id only means "my parent within this trace, recorded in
    // this ledger." A fresh run id makes a lingering LF_PROCESS_ID belong to
    // the old trace; and `ledger_insert` is best-effort, so a parent whose
    // write never landed exported its identity anyway. Both spell a parent
    // that resolves to nothing. Drop it so the violation is unspellable at
    // write time; the run id stays, so the trace still groups. A legitimate
    // parent records its own start row before it can spawn anything.
    let parent_process_id = (!minted_run_id)
        .then(|| {
            std::env::var(LF_PROCESS_ID_ENV)
                .ok()
                .and_then(|value| ExecId::parse(&value).ok())
        })
        .flatten()
        .filter(parent_is_recorded);
    let process_id = ExecId::default();
    std::env::set_var(LF_PROCESS_ID_ENV, process_id.as_str());

    // Write the file journal wherever we can. Fall back to ledger-only when
    // the journal can't be
    // git-excluded (e.g. not a git repo).
    let run_dir = match ensure_journal_ignored(repo_root) {
        Ok(()) => {
            let dir = runs_root(repo_root).join(run_id.as_str());
            fs::create_dir_all(&dir)?;
            Some(dir)
        }
        Err(err) => {
            debug!(
                error = %err,
                repo = %repo_root.display(),
                "file journal unavailable; recording to ledger only"
            );
            None
        }
    };

    let repo = main_repo
        .as_deref()
        .unwrap_or(repo_root)
        .display()
        .to_string();

    let context = RunContext {
        run_id,
        process_id,
        parent_process_id,
        command: fields
            .command
            .as_ref()
            .and_then(|argv| serde_json::to_string(argv).ok()),
        run_dir,
        repo: Some(repo),
        wave: wave_name.clone(),
        seq: 0,
        minted_run_id,
    };
    set_context(context.clone());
    if let Err(error) = write_exec_process_receipt(&context) {
        debug!(error = %error, exec_id = %context.process_id, "live Exec receipt unavailable");
    } else {
        crate::engine::agent::register_interrupt_cleanup(remove_exec_process_receipt);
    }

    if let Some(wave_name) = wave_name {
        if fields.wave_name.as_deref() != Some(wave_name.as_str()) {
            debug!(
                expected_wave = %wave_name,
                observed_wave = ?fields.wave_name,
                repo = %repo_root.display(),
                "journal run start received mismatched wave metadata"
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
fn parent_is_recorded(parent: &ExecId) -> bool {
    let path = match ledger_db_path() {
        Ok(path) if path.exists() => path,
        Ok(_) => return false,
        Err(err) => {
            debug!(error = %err, "no ledger path; keeping the inherited parent process id");
            return true;
        }
    };
    match SqliteStore::open_run_ledger_read_only(&path)
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

fn next_seq() -> i64 {
    RUN_CONTEXT.with(|cell| {
        let mut borrow = cell.borrow_mut();
        match borrow.as_mut() {
            Some(context) => {
                let seq = context.seq;
                context.seq += 1;
                seq
            }
            None => 0,
        }
    })
}

fn configured_run_id(repo_root: &Path) -> Option<TraceId> {
    let value = std::env::var(LF_TRACE_ID_ENV).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    match trimmed.parse() {
        Ok(run_id) => Some(run_id),
        Err(err) => {
            debug!(
                env = LF_TRACE_ID_ENV,
                value = trimmed,
                repo = %repo_root.display(),
                error = %err,
                "ignoring invalid journal run id override"
            );
            None
        }
    }
}

fn append_event(run_dir: &Path, event: &LfEvent) -> Result<(), std::io::Error> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_path(run_dir))?;
    let _lock = lock_file(&file)?;
    let mut line = serde_json::to_vec(event).map_err(std::io::Error::other)?;
    line.push(b'\n');
    file.write_all(&line)?;
    Ok(())
}

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

fn current_context() -> Option<RunContext> {
    RUN_CONTEXT.with(|cell| cell.borrow().clone())
}

fn set_context(context: RunContext) {
    RUN_CONTEXT.with(|cell| {
        *cell.borrow_mut() = Some(context);
    });
}

fn clear_context() {
    RUN_CONTEXT.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

pub(crate) fn read_exec_process_receipts_at(
    lf_home: &Path,
) -> Result<Vec<ExecProcessReceipt>, std::io::Error> {
    let root = lf_home.join(EXEC_PROCESS_ROOT);
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
        if path.extension().and_then(|value| value.to_str()) != Some("json")
            || path
                .file_stem()
                .and_then(|value| value.to_str())
                .and_then(|value| value.parse::<u32>().ok())
                .is_none()
        {
            continue;
        }
        let Ok(content) = fs::read(path) else {
            continue;
        };
        let Ok(receipt) = serde_json::from_slice::<ExecProcessReceipt>(&content) else {
            continue;
        };
        if receipt.schema_version == 1 {
            receipts.push(receipt);
        }
    }
    Ok(receipts)
}

pub(crate) fn current_exec_process_receipt() -> Result<ExecProcessReceipt, std::io::Error> {
    let pid = std::process::id();
    let path = crate::store::lf_home_dir()
        .join(EXEC_PROCESS_ROOT)
        .join(format!("{pid}.json"));
    let receipt: ExecProcessReceipt =
        serde_json::from_slice(&fs::read(&path)?).map_err(std::io::Error::other)?;
    let trace_id = std::env::var(LF_TRACE_ID_ENV)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::NotFound, error))?;
    let exec_id = std::env::var(LF_PROCESS_ID_ENV)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::NotFound, error))?;
    if receipt.schema_version != 1
        || receipt.pid != pid
        || receipt.trace_id != trace_id
        || receipt.exec_id != exec_id
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "live Exec receipt {} does not match this process",
                path.display()
            ),
        ));
    }
    Ok(receipt)
}

pub(crate) fn remove_exec_process_receipt_at(
    lf_home: &Path,
    pid: u32,
) -> Result<bool, std::io::Error> {
    let path = lf_home.join(EXEC_PROCESS_ROOT).join(format!("{pid}.json"));
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn write_exec_process_receipt(context: &RunContext) -> Result<(), std::io::Error> {
    let root = crate::store::lf_home_dir().join(EXEC_PROCESS_ROOT);
    fs::create_dir_all(&root)?;
    let pid = std::process::id();
    let receipt = ExecProcessReceipt {
        schema_version: 1,
        trace_id: context.run_id.to_string(),
        exec_id: context.process_id.to_string(),
        pid,
        started_at: OffsetDateTime::now_utc().unix_timestamp(),
    };
    let bytes = serde_json::to_vec(&receipt).map_err(std::io::Error::other)?;
    let path = root.join(format!("{pid}.json"));
    let temporary = root.join(format!(".{pid}.json.tmp"));
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)
}

fn remove_exec_process_receipt() {
    let path = crate::store::lf_home_dir()
        .join(EXEC_PROCESS_ROOT)
        .join(format!("{}.json", std::process::id()));
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            debug!(error = %error, "failed to remove live Exec receipt");
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
        emit, events_path, read_events, runs_root, LfEvent, LfEventFields, LfEventType, LfNode,
        TestLedgerGuard,
    };
    use crate::engine::git::is_clean;
    use crate::id::{ExecId, TraceId, WaveId};
    use crate::work::wave::Wave;
    use loopflow_test_support::TestRepo;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    const CHILD_APPEND_ENV: &str = "LOOPFLOW_JOURNAL_APPEND_CHILD";
    const CHILD_EVENT_COUNT_ENV: &str = "LOOPFLOW_JOURNAL_CHILD_EVENT_COUNT";
    const CHILD_RUN_DIR_ENV: &str = "LOOPFLOW_JOURNAL_CHILD_RUN_DIR";
    const CHILD_WRITER_ENV: &str = "LOOPFLOW_JOURNAL_CHILD_WRITER";

    struct AmbientStorage {
        _lock: std::sync::MutexGuard<'static, ()>,
        previous_lf_home: Option<std::ffi::OsString>,
        previous_db_path: Option<std::ffi::OsString>,
    }

    impl AmbientStorage {
        fn seed(home: &std::path::Path, db_path: &std::path::Path) -> Self {
            let lock = super::test_env_lock();
            let previous_lf_home = std::env::var_os("LF_HOME");
            let previous_db_path = std::env::var_os("LF_DB_PATH");
            std::env::set_var("LF_HOME", home);
            std::env::set_var("LF_DB_PATH", db_path);
            Self {
                _lock: lock,
                previous_lf_home,
                previous_db_path,
            }
        }
    }

    impl Drop for AmbientStorage {
        fn drop(&mut self) {
            match &self.previous_lf_home {
                Some(value) => std::env::set_var("LF_HOME", value),
                None => std::env::remove_var("LF_HOME"),
            }
            match &self.previous_db_path {
                Some(value) => std::env::set_var("LF_DB_PATH", value),
                None => std::env::remove_var("LF_DB_PATH"),
            }
        }
    }

    fn with_run_id_env<T>(value: Option<&str>, run: impl FnOnce() -> T) -> T {
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
        let ambient_db_dir = tempfile::tempdir().expect("ambient db dir");
        let ambient_db = ambient_db_dir.path().join("production.db");
        let _ambient = AmbientStorage::seed(ambient_home.path(), &ambient_db);

        let resolved = super::ledger_db_path().expect("test ledger path");
        super::open_ledger().expect("open test ledger");

        assert_ne!(resolved, ambient_db);
        assert_ne!(resolved, ambient_home.path().join("loopflow.db"));
        assert!(!ambient_db.exists());
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

    fn only_run_dir(worktree: &std::path::Path) -> std::path::PathBuf {
        let mut entries = std::fs::read_dir(runs_root(worktree))
            .expect("read runs")
            .map(|entry| entry.expect("run dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1, "expected a single journal run dir");
        entries.pop().expect("run dir")
    }

    #[test]
    fn concurrent_child_process_appends_keep_events_jsonl_parseable() {
        let tmp = tempfile::TempDir::new().expect("temp journal");
        let run_dir = tmp.path().join("run");
        std::fs::create_dir_all(&run_dir).expect("create run dir");

        let current_exe = std::env::current_exe().expect("current test binary");
        let writers = 8;
        let events_per_writer = 20;
        let mut children = Vec::new();
        for writer in 0..writers {
            let child = Command::new(&current_exe)
                .arg("journal_child_process_appends_events_for_concurrency_regression")
                .arg("--nocapture")
                .env(CHILD_APPEND_ENV, "1")
                .env(CHILD_RUN_DIR_ENV, &run_dir)
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

        let raw = std::fs::read_to_string(events_path(&run_dir)).expect("read events.jsonl");
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

        let parsed = read_events(&run_dir).expect("parse all events");
        assert_eq!(parsed.len(), writers * events_per_writer);
    }

    #[test]
    fn journal_child_process_appends_events_for_concurrency_regression() {
        if std::env::var(CHILD_APPEND_ENV).ok().as_deref() != Some("1") {
            return;
        }

        let run_dir = PathBuf::from(std::env::var(CHILD_RUN_DIR_ENV).expect("child run dir env"));
        let writer = std::env::var(CHILD_WRITER_ENV).expect("child writer env");
        let event_count = std::env::var(CHILD_EVENT_COUNT_ENV)
            .expect("child event count env")
            .parse::<usize>()
            .expect("child event count");
        let run_id = TraceId::parse("8985c55b-9864-4c2b-860f-b7054a71bbea").expect("run id");

        for index in 0..event_count {
            let event = LfEvent {
                run_id: run_id.clone(),
                ts: time::OffsetDateTime::now_utc(),
                node: LfNode::Skill,
                event: LfEventType::Errored,
                wave_name: Some("meta".to_string()),
                worktree: Some(run_dir.display().to_string()),
                command: None,
                flow: Some("garden".to_string()),
                skill: Some(format!("writer-{writer}-{index}")),
                index: Some(index as u32),
                error: Some(format!(
                    "writer-{writer}-event-{index}:{}",
                    "x".repeat(16 * 1024)
                )),
                signal: None,
            };
            super::append_event(&run_dir, &event).expect("child append event");
        }
    }

    #[test]
    fn main_repo_runs_record_to_file_journal_and_ledger() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let command = vec!["lf".to_string(), "implement".to_string()];

        emit(
            repo.path(),
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, repo.path(), "main"),
        );
        emit(
            repo.path(),
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        // The file journal exists in the main repo too — generic contexts
        // record as much as possible; only the wave field is absent.
        let run_dir = only_run_dir(repo.path());
        let file_events = read_events(&run_dir).expect("file events");
        assert_eq!(file_events.len(), 2);
        assert!(is_clean(repo.path()).expect("journal stays git-excluded"));

        // And the machine-grain ledger has the run's lineage and a null wave.
        // LF_HOME points this test at its own store, so every row here belongs
        // to this invocation.
        let store = super::open_ledger().expect("ledger");
        let events = store.list_run_events_since(0).expect("ledger rows");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].node, "run");
        assert_eq!(events[0].event, "started");
        assert!(events[0].repo.is_some());
        assert!(std::path::Path::new(events[0].repo.as_deref().unwrap()).is_absolute());
        assert_eq!(events[0].wave, None);
        assert!(events[0]
            .command
            .as_deref()
            .unwrap_or("")
            .contains("implement"));
        assert_eq!(events[1].event, "completed");
        assert_eq!(events[0].process_id, events[1].process_id);
        assert_eq!(events[1].command, events[0].command);
    }

    #[test]
    fn explicit_wave_env_overrides_the_worktree_for_ledger_attribution() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let worktree = repo.create_named_worktree("ambient");
        let wave = Wave::new(
            WaveId::new(),
            "context".to_string(),
            repo.path().display().to_string(),
        );
        super::open_ledger()
            .expect("ledger")
            .create_wave(&wave)
            .expect("explicit wave row");
        std::env::set_var(crate::work::wave::context::WAVE_ID_ENV, wave.id().as_str());

        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            started_fields(
                &["lf".to_string(), "design".to_string()],
                &worktree,
                "context",
            ),
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        let events = super::open_ledger()
            .expect("ledger")
            .list_run_events_since(0)
            .expect("events");
        assert_eq!(events[0].wave.as_deref(), Some("context"));
        std::env::remove_var(crate::work::wave::context::WAVE_ID_ENV);
    }

    /// W2-239: a stale ambient UUID (registry has no row for it) is propagated
    /// into the run record as a classified failure, never silently re-attributed
    /// to the worktree or a different wave. The run records `wave: None`
    /// (honest — no valid name) and the stale failure in the existing `error`
    /// field; the text names the stale id and the `--wave <name>` recovery.
    #[test]
    fn stale_ambient_uuid_is_propagated_not_inferred_from_the_worktree() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let worktree = repo.create_named_worktree("ambient");

        // A valid wave exists; the run's env names a different, unregistered UUID.
        let registered = Wave::new(
            WaveId::new(),
            "context".to_string(),
            repo.path().display().to_string(),
        );
        super::open_ledger()
            .expect("ledger")
            .create_wave(&registered)
            .expect("registered wave row");
        let stale_id = WaveId::new();
        std::env::set_var(crate::work::wave::context::WAVE_ID_ENV, stale_id.as_str());

        // `with_runtime` resolves once and records wave + failure; mirror that.
        let attribution = crate::work::wave::context::run_attribution(Some(&worktree));
        assert_eq!(
            attribution.wave, None,
            "stale identity attributes to no wave"
        );
        let failure = attribution
            .failure
            .clone()
            .expect("classified stale failure");
        assert!(failure.contains("stale"), "failure text: {failure}");
        assert!(
            failure.contains(stale_id.as_str()),
            "failure names the stale id: {failure}"
        );
        assert!(
            failure.contains("--wave"),
            "failure names the explicit recovery: {failure}"
        );

        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            LfEventFields {
                wave_name: attribution.wave,
                error: attribution.failure,
                worktree: Some(worktree.display().to_string()),
                command: Some(vec!["lf".to_string(), "design".to_string()]),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        let events = super::open_ledger()
            .expect("ledger")
            .list_run_events_since(0)
            .expect("events");
        let started = events
            .iter()
            .find(|row| row.node == "run" && row.event == "started")
            .expect("started row");
        // Attributed to NO wave — never the worktree or the registered wave —
        // and the stale failure rides the existing error field (honest wire).
        assert_eq!(started.wave, None);
        assert_eq!(started.error.as_deref(), Some(failure.as_str()));

        std::env::remove_var(crate::work::wave::context::WAVE_ID_ENV);
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
            LfNode::Run,
            LfEventType::Started,
            fields.clone(),
        );
        let parent = super::current_context().expect("parent");
        super::clear_context();
        let child = super::ensure_run_context(repo.path(), &fields)
            .expect("child context")
            .expect("child");

        assert_ne!(parent.process_id, child.process_id);
        assert_eq!(
            child.run_id, parent.run_id,
            "a nested lf stays in the trace"
        );
        assert_eq!(child.parent_process_id, Some(parent.process_id));
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

        // A real run first, so the ledger exists and holds rows. Without it the
        // drop proves nothing: an absent database answers "not recorded" for
        // every id, and this passes with the lookup hardwired to `true`.
        super::emit(
            repo.path(),
            LfNode::Run,
            LfEventType::Started,
            fields.clone(),
        );
        let recorded = super::current_context().expect("recorded run");
        super::clear_context();

        // A parent that exported its identity but never reached the ledger.
        let ghost = ExecId::new();
        std::env::set_var(super::LF_TRACE_ID_ENV, recorded.run_id.as_str());
        std::env::set_var(super::LF_PROCESS_ID_ENV, ghost.as_str());

        let context = super::ensure_run_context(repo.path(), &fields)
            .expect("run context")
            .expect("context");

        assert!(
            super::parent_is_recorded(&recorded.process_id),
            "the ledger must really hold a parent, or the assertion below \
             passes for the wrong reason"
        );

        assert_eq!(
            context.parent_process_id, None,
            "a parent the ledger never recorded is a ghost, not lineage"
        );
        assert!(
            !context.minted_run_id,
            "the trace still groups; only the false pointer goes"
        );

        super::clear_context();
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        std::env::remove_var(super::LF_TRACE_ID_ENV);
    }

    #[test]
    fn a_detached_body_inherits_a_recorded_parent_across_its_launcher() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let fields = started_fields(&["lf".to_string(), "task".to_string()], repo.path(), "main");

        // A detached body inherits LF_TRACE_ID/LF_PROCESS_ID from a launcher that
        // has already exited. The launcher's row outlives it, so the parent
        // still resolves and must survive the drop rule.
        super::emit(
            repo.path(),
            LfNode::Run,
            LfEventType::Started,
            fields.clone(),
        );
        let launcher = super::current_context().expect("launcher");
        super::emit(
            repo.path(),
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        // The body carries what the launcher handed it, not what the launcher
        // left behind: a terminal run clears LF_PROCESS_ID from the env.
        std::env::set_var(super::LF_TRACE_ID_ENV, launcher.run_id.as_str());
        std::env::set_var(super::LF_PROCESS_ID_ENV, launcher.process_id.as_str());
        super::clear_context();

        let body = super::ensure_run_context(repo.path(), &fields)
            .expect("body context")
            .expect("body");

        assert_eq!(body.parent_process_id, Some(launcher.process_id));
        assert_eq!(body.run_id, launcher.run_id);

        super::clear_context();
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        std::env::remove_var(super::LF_TRACE_ID_ENV);
    }

    #[test]
    fn minting_a_fresh_run_drops_a_stale_cross_trace_parent() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let fields = started_fields(
            &["lf".to_string(), "kickoff".to_string()],
            repo.path(),
            "main",
        );

        // A process id lingers in the environment but no run id does — the
        // `pr land` / `wt switch` / `kickoff` shape that historically stamped a
        // new trace with a parent from the old one.
        std::env::set_var(super::LF_PROCESS_ID_ENV, ExecId::new().as_str());

        let context = super::ensure_run_context(repo.path(), &fields)
            .expect("run context")
            .expect("context");

        assert!(context.minted_run_id, "no LF_TRACE_ID means a fresh trace");
        assert_eq!(
            context.parent_process_id, None,
            "a fresh trace has no in-trace parent to name"
        );
        super::clear_context();
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        std::env::remove_var(super::LF_TRACE_ID_ENV);
    }

    #[test]
    fn a_terminal_row_names_the_work_its_started_row_named() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let command = vec!["lf".to_string(), "gate".to_string()];

        emit(
            repo.path(),
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, repo.path(), "main"),
        );
        emit(
            repo.path(),
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        let events = super::open_ledger()
            .expect("ledger")
            .list_run_events_since(0)
            .expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].command, events[0].command);
        assert!(events[1].command.as_deref().unwrap_or("").contains("gate"));
    }

    #[test]
    fn journal_writes_run_flow_and_skill_events_in_wave_worktree() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let worktree = repo.create_named_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        emit(
            &worktree,
            LfNode::Flow,
            LfEventType::Started,
            LfEventFields {
                flow: Some("build".to_string()),
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
            LfNode::Flow,
            LfEventType::Completed,
            LfEventFields::default(),
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        let run_dir = only_run_dir(&worktree);
        let events = read_events(&run_dir).expect("read events");
        assert_eq!(events.len(), 6);
        assert_eq!(events[0].node, LfNode::Run);
        assert_eq!(events[0].event, LfEventType::Started);
        assert_eq!(events[0].wave_name.as_deref(), Some("runtime"));
        assert_eq!(events[1].node, LfNode::Flow);
        assert_eq!(events[1].flow.as_deref(), Some("build"));
        assert_eq!(events[2].node, LfNode::Skill);
        assert_eq!(events[2].skill.as_deref(), Some("implement"));
        assert_eq!(events[2].index, Some(0));
        assert_eq!(events[3].event, LfEventType::Completed);
        assert_eq!(events[4].node, LfNode::Flow);
        assert_eq!(events[5].node, LfNode::Run);
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
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );

        assert!(is_clean(&worktree).expect("worktree should stay clean"));
    }

    #[test]
    fn run_lifecycle_publishes_and_removes_exact_process_ownership() {
        let guard = journal_test_guard();
        let repo = TestRepo::new();
        let worktree = repo.create_named_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        let receipts =
            super::read_exec_process_receipts_at(guard.home()).expect("read live receipt");
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].pid, std::process::id());
        assert_eq!(
            receipts[0].exec_id,
            std::env::var(super::LF_PROCESS_ID_ENV).expect("current Exec id")
        );
        assert_eq!(
            super::current_exec_process_receipt().expect("read current live receipt"),
            receipts[0]
        );

        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );
        assert!(super::read_exec_process_receipts_at(guard.home())
            .expect("read terminal receipt state")
            .is_empty());
    }

    #[test]
    fn terminal_run_events_clear_context_for_the_next_run() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let worktree = repo.create_named_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        let entries = std::fs::read_dir(runs_root(&worktree))
            .expect("read runs")
            .count();
        assert_eq!(entries, 2);
    }

    #[test]
    fn journal_uses_configured_run_id_when_present() {
        with_run_id_env(Some("7c22895f-e4c1-49cc-a95d-2267e2356f16"), || {
            let repo = TestRepo::new();
            let worktree = repo.create_named_worktree("runtime");
            let command = vec!["lf".to_string(), "build".to_string()];

            emit(
                &worktree,
                LfNode::Run,
                LfEventType::Started,
                started_fields(&command, &worktree, "runtime"),
            );
            emit(
                &worktree,
                LfNode::Run,
                LfEventType::Completed,
                LfEventFields::default(),
            );

            let run_dir = only_run_dir(&worktree);
            let run_id = run_dir
                .file_name()
                .and_then(|name| name.to_str())
                .expect("run dir name");
            assert_eq!(run_id, "7c22895f-e4c1-49cc-a95d-2267e2356f16");
        });
    }

    #[test]
    fn invalid_configured_run_id_falls_back_to_generated_id() {
        with_run_id_env(Some("not-a-uuid"), || {
            let repo = TestRepo::new();
            let worktree = repo.create_named_worktree("runtime");
            let command = vec!["lf".to_string(), "build".to_string()];

            emit(
                &worktree,
                LfNode::Run,
                LfEventType::Started,
                started_fields(&command, &worktree, "runtime"),
            );
            emit(
                &worktree,
                LfNode::Run,
                LfEventType::Completed,
                LfEventFields::default(),
            );

            let run_dir = only_run_dir(&worktree);
            let run_id = run_dir
                .file_name()
                .and_then(|name| name.to_str())
                .expect("run dir name");
            assert!(
                TraceId::parse(run_id).is_ok(),
                "expected generated UUID run id"
            );
            assert_ne!(run_id, "not-a-uuid");
        });
    }
}
