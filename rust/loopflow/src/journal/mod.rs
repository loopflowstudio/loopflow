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
use crate::id::{ProcessId, RunId};
use crate::store::sqlite::SqliteStore;
use crate::store::RunEventRow;

const JOURNAL_ROOT: &str = ".lf/journal/runs";
const JOURNAL_EXCLUDE_ENTRY: &str = ".lf/journal/";
pub const LF_RUN_ID_ENV: &str = "LF_RUN_ID";
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
    static PENDING_USAGE: RefCell<PendingUsage> = const { RefCell::new(PendingUsage::new()) };
}

#[derive(Debug, Clone)]
struct RunContext {
    run_id: RunId,
    process_id: ProcessId,
    parent_process_id: Option<ProcessId>,
    /// Serialized argv captured at run start so terminal rows name their work.
    command: Option<String>,
    /// File-journal directory. Written in any git checkout; None only when the
    /// journal can't be git-excluded. The SQLite ledger records every run.
    run_dir: Option<PathBuf>,
    repo: Option<String>,
    wave: Option<String>,
    seq: i64,
    /// True when this process minted the run id (vs inheriting LF_RUN_ID);
    /// the export is removed again when the run ends.
    minted_run_id: bool,
}

/// Token/cost totals accumulated from the agent stream on this thread, plus the
/// agent that spent them. Attached to ledger rows as the run progresses.
#[derive(Debug, Clone)]
struct PendingUsage {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cost_usd: Option<f64>,
    duration_secs: Option<f64>,
    provider: Option<&'static str>,
    model: Option<String>,
    seen: bool,
}

impl PendingUsage {
    const fn new() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cost_usd: None,
            duration_secs: None,
            provider: None,
            model: None,
            seen: false,
        }
    }
}

/// Accumulate token usage reported by the agent stream for the current run.
pub fn record_usage(input: Option<u64>, output: Option<u64>, cache_read: Option<u64>) {
    PENDING_USAGE.with(|cell| {
        let mut usage = cell.borrow_mut();
        usage.input_tokens += input.unwrap_or(0);
        usage.output_tokens += output.unwrap_or(0);
        usage.cache_read_tokens += cache_read.unwrap_or(0);
        usage.seen = true;
    });
}

/// Record the stream's final cost/duration report for the current run.
///
/// Every usage field on a ledger row is cumulative to that point in the run, so
/// a reader diffs consecutive rows for a per-skill figure and reads the terminal
/// row for the run total. Cost used to overwrite instead of accumulate, which
/// made a multi-skill run report only its last agent invocation's cost — and a
/// run's cost could *fall* between skills, which no running total ever does.
pub fn record_result(cost_usd: Option<f64>, duration_secs: Option<f64>) {
    PENDING_USAGE.with(|cell| {
        let mut usage = cell.borrow_mut();
        if let Some(cost) = cost_usd {
            usage.cost_usd = Some(usage.cost_usd.unwrap_or(0.0) + cost);
        }
        if let Some(duration) = duration_secs {
            usage.duration_secs = Some(usage.duration_secs.unwrap_or(0.0) + duration);
        }
        usage.seen = true;
    });
}

/// Name the harness the current agent launch is spending tokens through, and
/// the model it drove. Recorded without marking usage seen — a launch that
/// reports no tokens names an agent but should not materialize a row.
///
/// Set both fields together. A process may launch several agents, and an
/// unconfigured model on the second launch must clear the first launch's model
/// rather than producing a fictitious `codex:opus` boundary.
pub fn record_agent(provider: Option<&'static str>, model: Option<&str>) {
    PENDING_USAGE.with(|cell| {
        let mut usage = cell.borrow_mut();
        usage.provider = provider;
        usage.model = model.map(str::to_string);
    });
}

fn snapshot_usage() -> Option<PendingUsage> {
    PENDING_USAGE.with(|cell| {
        let usage = cell.borrow();
        usage.seen.then(|| usage.clone())
    })
}

fn clear_usage() {
    PENDING_USAGE.with(|cell| {
        *cell.borrow_mut() = PendingUsage::new();
    });
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
    pub run_id: RunId,
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
        if context.minted_run_id {
            std::env::remove_var(LF_RUN_ID_ENV);
        }
        std::env::remove_var(LF_PROCESS_ID_ENV);
        clear_context();
        clear_usage();
    }

    Ok(())
}

/// Best-effort write into the machine-grain SQLite ledger. Never fails the
/// run: a locked or missing store degrades to a debug log line. Local-only —
/// the ledger never leaves the machine.
fn ledger_insert(context: &RunContext, event: &LfEvent, seq: i64, repo_root: &Path) {
    let is_terminal_run = matches!(event.node, LfNode::Run)
        && matches!(
            event.event,
            LfEventType::Completed | LfEventType::Errored | LfEventType::Escalated
        );
    let is_skill_boundary = matches!(event.node, LfNode::Skill)
        && matches!(event.event, LfEventType::Completed | LfEventType::Errored);
    // Terminal run rows carry the run's totals; skill boundaries carry a
    // cumulative snapshot so a reader can diff consecutive skills.
    let usage = if is_terminal_run || is_skill_boundary {
        snapshot_usage()
    } else {
        None
    };

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
        input_tokens: usage.as_ref().map(|u| u.input_tokens as i64),
        output_tokens: usage.as_ref().map(|u| u.output_tokens as i64),
        cache_read_tokens: usage.as_ref().map(|u| u.cache_read_tokens as i64),
        cost_usd: usage.as_ref().and_then(|u| u.cost_usd),
        duration_secs: usage.as_ref().and_then(|u| u.duration_secs),
        provider: usage.as_ref().and_then(|u| u.provider).map(str::to_string),
        model: usage.as_ref().and_then(|u| u.model.clone()),
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

/// Return explicit launch identity for durable trace capture.
pub fn trace_capture_context(
    worktree: &Path,
    flow: Option<String>,
    skill: Option<String>,
) -> Option<crate::trace::TraceCaptureContext> {
    let context = current_context()?;
    let (project, task) = child_work_attribution();
    Some(crate::trace::TraceCaptureContext {
        run_id: context.run_id,
        process_id: context.process_id,
        repo: context
            .repo
            .map(PathBuf::from)
            .unwrap_or_else(|| worktree.to_path_buf()),
        worktree: worktree.to_path_buf(),
        wave: context.wave,
        project,
        task,
        flow,
        skill,
    })
}

fn child_work_attribution() -> (Option<String>, Option<String>) {
    let Ok(store) = open_ledger() else {
        return (None, None);
    };
    if let Some(value) = std::env::var_os("LF_TASK_SESSION_ID") {
        if let Ok(id) = crate::task::TaskSessionId::parse(&value.to_string_lossy()) {
            if let Ok(Some(session)) = store.task_session(&id) {
                return (
                    Some(session.launch.project.slug),
                    Some(session.launch.issue.identifier),
                );
            }
        }
    }
    if let Some(value) = std::env::var_os("LF_PROJECT_SESSION_ID") {
        if let Ok(id) = crate::project_session::ProjectSessionId::parse(&value.to_string_lossy()) {
            if let Ok(Some(session)) = store.project_session(&id) {
                return (Some(session.launch.project.slug), None);
            }
        }
    }
    (None, None)
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
    let attribution = crate::engine::wave_context::run_attribution();
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
            let run_id = RunId::default();
            std::env::set_var(LF_RUN_ID_ENV, run_id.as_str());
            (run_id, true)
        }
    };

    // A parent process id only means "my parent within this trace." When we
    // mint a fresh run id we are starting a new trace, so a lingering
    // LF_PROCESS_ID belongs to the old one — carrying it forward would stamp
    // this row with a cross-trace parent that `lf doctor`'s lineage check
    // (rightly) rejects. Drop it so the violation is unspellable at write time.
    let parent_process_id = (!minted_run_id)
        .then(|| {
            std::env::var(LF_PROCESS_ID_ENV)
                .ok()
                .and_then(|value| ProcessId::parse(&value).ok())
        })
        .flatten();
    let process_id = ProcessId::default();
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

fn configured_run_id(repo_root: &Path) -> Option<RunId> {
    let value = std::env::var(LF_RUN_ID_ENV).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    match trimmed.parse() {
        Ok(run_id) => Some(run_id),
        Err(err) => {
            debug!(
                env = LF_RUN_ID_ENV,
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
    #[test]
    fn every_usage_field_accumulates_across_a_multi_skill_run() {
        // A ledger row's usage is cumulative to that point. Cost once
        // overwrote, so a run could report a *lower* cost after a later skill
        // and `lf usage` summed only the final skill's spend.
        super::clear_usage();
        super::record_usage(Some(100), Some(10), Some(5));
        super::record_result(Some(1.00), Some(2.0));
        super::record_usage(Some(50), Some(5), Some(0));
        super::record_result(Some(0.25), Some(3.0));

        let usage = super::snapshot_usage().expect("usage seen");
        assert_eq!(usage.input_tokens, 150);
        assert_eq!(usage.output_tokens, 15);
        assert_eq!(usage.cache_read_tokens, 5);
        assert_eq!(usage.duration_secs, Some(5.0));
        assert_eq!(
            usage.cost_usd,
            Some(1.25),
            "cost must accumulate, not overwrite"
        );
        super::clear_usage();
    }

    #[test]
    fn each_agent_launch_replaces_provider_and_model_attribution() {
        super::clear_usage();
        super::record_agent(Some("claude"), Some("opus"));
        super::record_usage(Some(100), Some(10), None);
        let first = super::snapshot_usage().expect("first usage");
        assert_eq!(first.provider, Some("claude"));
        assert_eq!(first.model.as_deref(), Some("opus"));

        super::record_agent(Some("codex"), None);
        super::record_usage(Some(50), Some(5), None);
        let second = super::snapshot_usage().expect("second usage");
        assert_eq!(second.provider, Some("codex"));
        assert_eq!(second.model, None, "the prior launch's model must not leak");
        super::clear_usage();
    }

    use super::{
        emit, events_path, read_events, runs_root, LfEvent, LfEventFields, LfEventType, LfNode,
        TestLedgerGuard,
    };
    use crate::engine::git::is_clean;
    use crate::id::{ProcessId, RunId, WaveId};
    use crate::wave::Wave;
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
        super::clear_usage();
        let previous = std::env::var(super::LF_RUN_ID_ENV).ok();
        let previous_process = std::env::var(super::LF_PROCESS_ID_ENV).ok();
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        match value {
            Some(value) => std::env::set_var(super::LF_RUN_ID_ENV, value),
            None => std::env::remove_var(super::LF_RUN_ID_ENV),
        }
        let result = run();
        super::clear_context();
        match previous {
            Some(value) => std::env::set_var(super::LF_RUN_ID_ENV, value),
            None => std::env::remove_var(super::LF_RUN_ID_ENV),
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
        super::clear_usage();
        std::env::remove_var(super::LF_RUN_ID_ENV);
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
        let run_id = RunId::parse("8985c55b-9864-4c2b-860f-b7054a71bbea").expect("run id");

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
        super::record_usage(Some(100), Some(20), Some(5));
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

        // And the machine-grain ledger has the run, with usage on the
        // terminal event and a null wave. LF_HOME points this test at its own
        // store, so every row here belongs to this invocation.
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
        assert_eq!(events[1].input_tokens, Some(100));
        assert_eq!(events[1].output_tokens, Some(20));
        assert_eq!(events[1].cache_read_tokens, Some(5));
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
        std::env::set_var(crate::engine::wave_context::WAVE_ID_ENV, wave.id().as_str());

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
        std::env::remove_var(crate::engine::wave_context::WAVE_ID_ENV);
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
        std::env::set_var(crate::engine::wave_context::WAVE_ID_ENV, stale_id.as_str());

        // `with_runtime` resolves once and records wave + failure; mirror that.
        let attribution = crate::engine::wave_context::run_attribution();
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

        std::env::remove_var(crate::engine::wave_context::WAVE_ID_ENV);
    }

    #[test]
    fn a_nested_lf_gets_its_own_span_and_names_its_parent() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let fields = started_fields(&["lf".to_string(), "wave".to_string()], repo.path(), "main");

        let parent = super::ensure_run_context(repo.path(), &fields)
            .expect("parent context")
            .expect("parent");
        super::clear_context();
        let child = super::ensure_run_context(repo.path(), &fields)
            .expect("child context")
            .expect("child");

        assert_ne!(parent.process_id, child.process_id);
        assert_eq!(child.parent_process_id, Some(parent.process_id));
        super::clear_context();
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        std::env::remove_var(super::LF_RUN_ID_ENV);
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
        std::env::set_var(super::LF_PROCESS_ID_ENV, ProcessId::new().as_str());

        let context = super::ensure_run_context(repo.path(), &fields)
            .expect("run context")
            .expect("context");

        assert!(context.minted_run_id, "no LF_RUN_ID means a fresh trace");
        assert_eq!(
            context.parent_process_id, None,
            "a fresh trace has no in-trace parent to name"
        );
        super::clear_context();
        std::env::remove_var(super::LF_PROCESS_ID_ENV);
        std::env::remove_var(super::LF_RUN_ID_ENV);
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
                RunId::parse(run_id).is_ok(),
                "expected generated UUID run id"
            );
            assert_ne!(run_id, "not-a-uuid");
        });
    }
}
