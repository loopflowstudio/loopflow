use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::debug;

use crate::engine::worktrees::{main_repo_root, wave_name_from_worktree_and_main};
use crate::lfd::id::LfdId;
use crate::lfd::store::sqlite::SqliteStore;
use crate::lfd::store::RunEventRow;

const JOURNAL_ROOT: &str = ".lf/journal/runs";
const JOURNAL_EXCLUDE_ENTRY: &str = ".lf/journal/";
pub const LF_RUN_ID_ENV: &str = "LF_RUN_ID";

thread_local! {
    static RUN_CONTEXT: RefCell<Option<RunContext>> = const { RefCell::new(None) };
    static PENDING_USAGE: RefCell<PendingUsage> = const { RefCell::new(PendingUsage::new()) };
}

#[derive(Debug, Clone)]
struct RunContext {
    run_id: LfdId,
    /// File-journal directory — present only in wave worktrees, where the
    /// daemon's poller tails it. The SQLite ledger records every run.
    run_dir: Option<PathBuf>,
    repo: Option<String>,
    wave: Option<String>,
    seq: i64,
    /// True when this process minted the run id (vs inheriting LF_RUN_ID);
    /// the export is removed again when the run ends.
    minted_run_id: bool,
}

/// Token/cost totals accumulated from the agent stream on this thread,
/// attached to ledger rows as the run progresses.
#[derive(Debug, Clone, Copy, Default)]
struct PendingUsage {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cost_usd: Option<f64>,
    duration_secs: Option<f64>,
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
pub fn record_result(cost_usd: Option<f64>, duration_secs: Option<f64>) {
    PENDING_USAGE.with(|cell| {
        let mut usage = cell.borrow_mut();
        if cost_usd.is_some() {
            usage.cost_usd = cost_usd;
        }
        if duration_secs.is_some() {
            usage.duration_secs = match usage.duration_secs {
                Some(existing) => Some(existing + duration_secs.unwrap_or(0.0)),
                None => duration_secs,
            };
        }
        usage.seen = true;
    });
}

fn snapshot_usage() -> Option<PendingUsage> {
    PENDING_USAGE.with(|cell| {
        let usage = *cell.borrow();
        usage.seen.then_some(usage)
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
    Step,
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
    pub step: Option<String>,
    pub index: Option<u32>,
    pub error: Option<String>,
    pub signal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LfEvent {
    pub run_id: LfdId,
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
    pub step: Option<String>,
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
        step: fields.step,
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
    let is_step_boundary = matches!(event.node, LfNode::Step)
        && matches!(event.event, LfEventType::Completed | LfEventType::Errored);
    // Terminal run rows carry the run's totals; step boundaries carry a
    // cumulative snapshot so a reader can diff consecutive steps.
    let usage = if is_terminal_run || is_step_boundary {
        snapshot_usage()
    } else {
        None
    };

    let row = RunEventRow {
        run_id: event.run_id.as_str().to_string(),
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
            .and_then(|argv| serde_json::to_string(argv).ok()),
        flow: event.flow.clone(),
        step: event.step.clone(),
        step_index: event.index.map(i64::from),
        error: event.error.clone(),
        input_tokens: usage.map(|u| u.input_tokens as i64),
        output_tokens: usage.map(|u| u.output_tokens as i64),
        cache_read_tokens: usage.map(|u| u.cache_read_tokens as i64),
        cost_usd: usage.and_then(|u| u.cost_usd),
        duration_secs: usage.and_then(|u| u.duration_secs),
    };

    match open_ledger() {
        Ok(store) => {
            if let Err(err) = store.insert_run_event(&row) {
                debug!(error = %err, run_id = %row.run_id, "ledger insert failed");
            }
        }
        Err(err) => {
            debug!(error = %err, "ledger unavailable");
        }
    }
}

/// Open the local ledger store, creating and migrating it if needed.
pub fn open_ledger() -> Result<SqliteStore, crate::lfd::store::StoreError> {
    SqliteStore::new(&crate::lfd::default_db_path())
}

fn node_name(node: LfNode) -> &'static str {
    match node {
        LfNode::Run => "run",
        LfNode::Flow => "flow",
        LfNode::Step => "step",
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
    let wave_name = main_repo
        .as_ref()
        .and_then(|main| wave_name_from_worktree_and_main(repo_root, main));

    let (run_id, minted_run_id) = match configured_run_id(repo_root) {
        Some(run_id) => (run_id, false),
        None => {
            // Mint and export the run id so prompt logs and child processes
            // carry the same identity as the ledger rows. The export is
            // removed when the run ends (see try_emit).
            let run_id = LfdId::default();
            std::env::set_var(LF_RUN_ID_ENV, run_id.as_str());
            (run_id, true)
        }
    };

    // The file journal exists for the daemon's poller, which only watches
    // wave worktrees. Everything else still lands in the SQLite ledger.
    let run_dir = if wave_name.is_some() {
        ensure_journal_ignored(repo_root)?;
        let dir = runs_root(repo_root).join(run_id.as_str());
        fs::create_dir_all(&dir)?;
        Some(dir)
    } else {
        None
    };

    let repo = main_repo
        .as_deref()
        .unwrap_or(repo_root)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);

    let context = RunContext {
        run_id,
        run_dir,
        repo,
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

fn configured_run_id(repo_root: &Path) -> Option<LfdId> {
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
    serde_json::to_writer(&mut file, event).map_err(std::io::Error::other)?;
    file.write_all(b"\n")?;
    Ok(())
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

    let exclude_path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
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
    use super::{emit, read_events, runs_root, LfEventFields, LfEventType, LfNode};
    use crate::engine::git::is_clean;
    use crate::lfd::id::LfdId;
    use loopflow_test_support::TestRepo;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_run_id_env<T>(value: Option<&str>, run: impl FnOnce() -> T) -> T {
        let _guard = env_lock().lock().expect("env lock");
        super::clear_context();
        super::clear_usage();
        let home = tempfile::TempDir::new().expect("ledger home");
        std::env::set_var("LF_HOME", home.path());
        let previous = std::env::var(super::LF_RUN_ID_ENV).ok();
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
        result
    }

    /// Holds the env lock and points the ledger (LF_HOME) at a tempdir so
    /// tests never touch the real ~/.lf store.
    fn journal_test_guard() -> (std::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
        let guard = env_lock().lock().expect("env lock");
        super::clear_context();
        super::clear_usage();
        std::env::remove_var(super::LF_RUN_ID_ENV);
        let home = tempfile::TempDir::new().expect("ledger home");
        std::env::set_var("LF_HOME", home.path());
        (guard, home)
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
    fn file_journal_is_disabled_in_main_repo_but_ledger_records() {
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

        // No file journal outside wave worktrees...
        assert!(!runs_root(repo.path()).exists());

        // ...but the machine-grain ledger has the run, with usage on the
        // terminal event.
        let store = super::open_ledger().expect("ledger");
        let events = store.list_run_events_since(0).expect("ledger rows");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].node, "run");
        assert_eq!(events[0].event, "started");
        assert!(events[0].repo.is_some());
        assert!(events[0]
            .command
            .as_deref()
            .unwrap_or("")
            .contains("implement"));
        assert_eq!(events[1].event, "completed");
        assert_eq!(events[1].input_tokens, Some(100));
        assert_eq!(events[1].output_tokens, Some(20));
        assert_eq!(events[1].cache_read_tokens, Some(5));
    }

    #[test]
    fn journal_writes_run_flow_and_step_events_in_wave_worktree() {
        let _guard = journal_test_guard();
        let repo = TestRepo::new();
        let worktree = repo.create_wave_worktree("runtime");
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
            LfNode::Step,
            LfEventType::Started,
            LfEventFields {
                step: Some("implement".to_string()),
                index: Some(0),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Step,
            LfEventType::Completed,
            LfEventFields {
                step: Some("implement".to_string()),
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
        assert_eq!(events[2].node, LfNode::Step);
        assert_eq!(events[2].step.as_deref(), Some("implement"));
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
        let worktree = repo.create_wave_worktree("runtime");
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
        let worktree = repo.create_wave_worktree("runtime");
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
            let worktree = repo.create_wave_worktree("runtime");
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
            let worktree = repo.create_wave_worktree("runtime");
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
                LfdId::parse(run_id).is_ok(),
                "expected generated UUID run id"
            );
            assert_ne!(run_id, "not-a-uuid");
        });
    }
}
