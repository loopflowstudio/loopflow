//! What happened in a Session lives in `history.db` beside `loopflow.db`:
//! observations a person reads later (`session_history`, 90 days) and one
//! row per provider turn (`turns`, kept). It has its own write lock, WAL and
//! revision row. A write never takes the store's write lock, and one that
//! fails is a [`RecordingGap`] in `logs/lf.log`, never a failed turn. Nothing
//! the product acts on lives here: deleting the file loses history and usage,
//! not state.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::{RecordingGap, StoreResult};

const FILE: &str = "history.db";
/// The schema store connections attach the file as.
const ALIAS: &str = "history";
const VERSION: i64 = 2;

/// The file's schema. It only adds: a new version creates what it lacks and
/// nothing migrates old rows. `{p}` names the schema the objects go in: empty
/// for the file itself, `history.` for the empty stand-in a store attaches
/// when it cannot reach the file.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS {p}session_history (
    seq INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    receipt_key TEXT NOT NULL,
    lf_process_id TEXT,
    task_id TEXT,
    wave_id TEXT,
    captured_event INTEGER,
    observed_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    -- What the row records, named once from its receipt key: every writer,
    -- and the retention pass moving old rows, write it by inserting.
    kind TEXT GENERATED ALWAYS AS (CASE
        WHEN receipt_key GLOB 'provider-transition:*' OR receipt_key GLOB 'agent-session:*'
            THEN 'transition'
        WHEN receipt_key GLOB 'lineage:*' THEN 'lineage'
        WHEN receipt_key GLOB 'attachment:*:exit' THEN 'departure'
        WHEN receipt_key GLOB '*:manifest.json' THEN 'manifest'
        WHEN receipt_key GLOB '*:terminal.json' THEN 'terminal'
        WHEN receipt_key GLOB '*:events.jsonl:*' THEN 'events'
        WHEN receipt_key GLOB '*:silence:*' THEN 'silence'
        WHEN receipt_key GLOB '*:store-gap:*' THEN 'gap'
        WHEN receipt_key GLOB '*:runs' THEN 'runs'
        ELSE 'other' END) STORED,
    UNIQUE(session_id, receipt_key)
);
CREATE INDEX IF NOT EXISTS {p}session_history_order ON session_history(session_id, seq);
CREATE INDEX IF NOT EXISTS {p}session_history_input ON session_history(captured_event)
    WHERE captured_event IS NOT NULL;
CREATE INDEX IF NOT EXISTS {p}session_history_outcome_window ON session_history(observed_at)
    WHERE kind IN ('manifest','terminal');
CREATE INDEX IF NOT EXISTS {p}session_history_window ON session_history(observed_at);
CREATE INDEX IF NOT EXISTS {p}session_history_process ON session_history(lf_process_id, session_id)
    WHERE lf_process_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS {p}session_history_task ON session_history(task_id)
    WHERE task_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS {p}session_history_kind ON session_history(session_id, kind, seq);

CREATE TABLE IF NOT EXISTS {p}turns (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    provider_thread TEXT NOT NULL,
    provider_turn TEXT NOT NULL,
    captured_event INTEGER,
    lf_process_id TEXT,
    agent_process_id TEXT,
    task_id TEXT,
    wave_id TEXT,
    project_id TEXT,
    repository TEXT,
    skill TEXT,
    graph TEXT,
    step TEXT,
    started_at INTEGER,
    ended_at INTEGER,
    status TEXT,
    duration_ms INTEGER,
    error TEXT,
    provider TEXT,
    model TEXT,
    usage_at INTEGER,
    usage_final INTEGER,
    usage_partial INTEGER,
    input_tokens INTEGER,
    cached_input_tokens INTEGER,
    cache_write_tokens INTEGER,
    output_tokens INTEGER,
    reasoning_tokens INTEGER,
    total_input_tokens INTEGER,
    peak_input_tokens INTEGER,
    context_window_tokens INTEGER,
    cost_usd REAL,
    first_request_tokens INTEGER,
    peak_request_tokens INTEGER,
    tool_results INTEGER,
    tool_output_tokens INTEGER,
    compactions INTEGER,
    compaction_tokens INTEGER,
    final_message TEXT,
    UNIQUE(session_id, provider_thread, provider_turn)
);
CREATE INDEX IF NOT EXISTS {p}turns_input ON turns(captured_event)
    WHERE captured_event IS NOT NULL;
CREATE INDEX IF NOT EXISTS {p}turns_started ON turns(started_at);
CREATE INDEX IF NOT EXISTS {p}turns_ended ON turns(ended_at) WHERE ended_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS {p}turns_process ON turns(lf_process_id, session_id)
    WHERE lf_process_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS {p}turns_task ON turns(task_id) WHERE task_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS {p}session_export_changes (
    position INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL UNIQUE
);
CREATE TRIGGER IF NOT EXISTS {p}export_turns_insert AFTER INSERT ON turns
BEGIN
    INSERT INTO session_export_changes(session_id) VALUES(NEW.session_id)
        ON CONFLICT(session_id) DO UPDATE SET position=(SELECT MAX(position)+1 FROM session_export_changes);
END;
CREATE TRIGGER IF NOT EXISTS {p}export_turns_update AFTER UPDATE ON turns
BEGIN
    INSERT INTO session_export_changes(session_id) VALUES(NEW.session_id)
        ON CONFLICT(session_id) DO UPDATE SET position=(SELECT MAX(position)+1 FROM session_export_changes);
END;
CREATE TRIGGER IF NOT EXISTS {p}export_session_history_insert AFTER INSERT ON session_history
BEGIN
    INSERT INTO session_export_changes(session_id) VALUES(NEW.session_id)
        ON CONFLICT(session_id) DO UPDATE SET position=(SELECT MAX(position)+1 FROM session_export_changes);
END;
CREATE TABLE IF NOT EXISTS {p}history_revision (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    revision INTEGER NOT NULL
);
INSERT OR IGNORE INTO {p}history_revision(id, revision) VALUES(1, 0);
CREATE TRIGGER IF NOT EXISTS {p}history_revision_session_insert AFTER INSERT ON session_history
BEGIN UPDATE history_revision SET revision = revision + 1; END;
CREATE TRIGGER IF NOT EXISTS {p}history_revision_session_delete AFTER DELETE ON session_history
BEGIN UPDATE history_revision SET revision = revision + 1; END;
CREATE TRIGGER IF NOT EXISTS {p}history_revision_turn_insert AFTER INSERT ON turns
BEGIN UPDATE history_revision SET revision = revision + 1; END;
CREATE TRIGGER IF NOT EXISTS {p}history_revision_turn_update AFTER UPDATE ON turns
BEGIN UPDATE history_revision SET revision = revision + 1; END;
CREATE TRIGGER IF NOT EXISTS {p}history_revision_turn_delete AFTER DELETE ON turns
BEGIN UPDATE history_revision SET revision = revision + 1; END;
";

fn schema(prefix: &str) -> String {
    SCHEMA.replace("{p}", prefix)
}

/// `history.db` in the directory of the store at `store`.
pub fn history_path(store: &Path) -> PathBuf {
    store.with_file_name(FILE)
}

/// One observation appended to `history.session_history`.
#[derive(Debug, Clone)]
pub(crate) struct Row {
    pub session_id: String,
    pub receipt_key: String,
    pub lf_process_id: Option<String>,
    pub task_id: Option<String>,
    pub wave_id: Option<String>,
    pub captured_event: Option<i64>,
    pub observed_at: i64,
    pub payload: String,
}

impl Row {
    /// An observation of `session` keyed `receipt_key`, belonging to input
    /// `captured_event`.
    pub fn observed(
        session: &str,
        receipt_key: String,
        captured_event: Option<i64>,
        observed_at: i64,
        payload: String,
    ) -> Self {
        Self {
            session_id: session.into(),
            receipt_key,
            lf_process_id: None,
            task_id: None,
            wave_id: None,
            captured_event,
            observed_at,
            payload,
        }
    }
}

/// Append `row` and return its sequence; a receipt already kept answers with
/// its own, and one kept with another payload is conflicting evidence.
pub(crate) fn append_in(tx: &Connection, row: &Row) -> StoreResult<i64> {
    let kept: Option<(i64, String)> = tx
        .query_row(
            "SELECT seq,payload FROM session_history WHERE session_id=?1 AND receipt_key=?2",
            rusqlite::params![row.session_id, row.receipt_key],
            |found| Ok((found.get(0)?, found.get(1)?)),
        )
        .optional()?;
    if let Some((seq, payload)) = kept {
        if payload == row.payload {
            return Ok(seq);
        }
        return Err(crate::StoreError::InvalidData(format!(
            "conflicting {} receipt for Session {}",
            row.receipt_key, row.session_id
        )));
    }
    tx.execute(
        "INSERT INTO session_history(session_id,receipt_key,lf_process_id,task_id,wave_id,
            captured_event,observed_at,payload)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![
            row.session_id,
            row.receipt_key,
            row.lf_process_id,
            row.task_id,
            row.wave_id,
            row.captured_event,
            row.observed_at,
            row.payload
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

/// A history write waits this long for another writer of the file, then
/// becomes a gap: a turn boundary does not queue behind history.
/// While a gap is open the next write does not wait at all, so a held lock
/// costs a process one wait, not one per write. Against a lock that
/// is never released, SQLite's WAL retries make that wait about 5 s.
#[cfg(not(test))]
const BUSY: Duration = Duration::from_secs(2);
#[cfg(test)]
const BUSY: Duration = Duration::from_millis(50);

/// The writer of `history.db`, shared by every store this process opens
/// beside it: opened at its first write, and reopened after a failure. `store`
/// is the store beside which the file lives; a store without a file keeps
/// none. Sharing it means a held lock costs the process one wait, however many
/// times it opens the store.
#[derive(Debug)]
pub struct HistoryFile {
    store: Option<PathBuf>,
    state: Mutex<WriterState>,
}

#[derive(Debug, Default)]
struct WriterState {
    conn: Option<Connection>,
    gap: RecordingGap,
}

impl HistoryFile {
    /// This process's writer of the history beside the store at `store`, or a
    /// writer of none for a store without a file.
    pub(crate) fn beside(store: Option<&Path>) -> Arc<Self> {
        static WRITERS: OnceLock<Mutex<HashMap<PathBuf, Arc<HistoryFile>>>> = OnceLock::new();
        let writer = |store: Option<&Path>| {
            Arc::new(Self {
                store: store.map(Path::to_path_buf),
                state: Mutex::default(),
            })
        };
        let Some(store) = store else {
            return writer(None);
        };
        let mut writers = WRITERS
            .get_or_init(Mutex::default)
            .lock()
            .expect("history writers mutex poisoned");
        // One file reached through two spellings (`/tmp` and `/private/tmp`)
        // still has one writer.
        let path = history_path(store);
        let key = path
            .parent()
            .and_then(|dir| dir.canonicalize().ok())
            .map_or_else(|| path.clone(), |dir| dir.join(FILE));
        writers
            .entry(key)
            .or_insert_with(|| writer(Some(store)))
            .clone()
    }

    /// Run `write` in one transaction on `history.db`, with the store
    /// attached read-only as `store`. Returns what it committed; a failure is
    /// a recording gap of the file, logged once however many kinds of write
    /// meet it, with `what` each missed at debug.
    pub(crate) fn write<T>(
        &self,
        what: &str,
        write: impl FnOnce(&Transaction<'_>) -> StoreResult<T>,
    ) -> Option<T> {
        let mut state = self.state.lock().expect("history mutex poisoned");
        let Some(store) = &self.store else {
            tracing::debug!("history write missed: {what}");
            state
                .gap
                .note(FILE, &"the store has no file beside which to keep it");
            return None;
        };
        let busy = if state.gap.span().is_some() {
            Duration::ZERO
        } else {
            BUSY
        };
        let result = commit(&mut state.conn, store, busy, write);
        match result {
            Ok(value) => {
                state.gap.recovered();
                Some(value)
            }
            Err(error) => {
                // A connection that failed mid-write is reopened next time.
                state.conn = None;
                tracing::debug!("history write missed: {what}");
                state.gap.note(FILE, &error);
                None
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn gap(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .gap
            .span()
            .map(|span| span.what.clone())
    }
}

fn commit<T>(
    conn: &mut Option<Connection>,
    store: &Path,
    busy: Duration,
    write: impl FnOnce(&Transaction<'_>) -> StoreResult<T>,
) -> StoreResult<T> {
    if conn.is_none() {
        *conn = Some(open_writer(store)?);
    }
    let conn = conn.as_mut().expect("history connection was just opened");
    conn.busy_timeout(busy)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let value = write(&tx)?;
    tx.commit()?;
    Ok(value)
}

/// Attach the history of the store at `store` to `conn` read-only as
/// `history`, so readers name `history.session_history` and `history.turns`,
/// making a missing file and its schema first. A file a store cannot reach is
/// a recording gap, never a failed open: readers then see it empty.
pub(crate) fn attach(conn: &Connection, store: &Path) {
    let path = history_path(store);
    let attached = (|| -> StoreResult<()> {
        if !path.exists() {
            open_writer(store)?;
        }
        conn.execute(&format!("ATTACH ?1 AS {ALIAS}"), [read_only_uri(&path)])?;
        Ok(())
    })();
    if let Err(error) = attached {
        RecordingGap::default().note(FILE, &error);
        attach_empty(conn);
    }
}

/// An empty in-memory history, for a store without a file beside which to
/// keep it.
pub(crate) fn attach_empty(conn: &Connection) {
    // A read-only store connection is `query_only`; its empty history is not
    // a write to anything the person keeps.
    let attached = (|| -> StoreResult<()> {
        let query_only: bool = conn.pragma_query_value(None, "query_only", |row| row.get(0))?;
        conn.execute_batch(&format!(
            "PRAGMA query_only=OFF; ATTACH ':memory:' AS {ALIAS}; {} PRAGMA query_only={};",
            schema(&format!("{ALIAS}.")),
            if query_only { "ON" } else { "OFF" },
        ))?;
        Ok(())
    })();
    if let Err(error) = attached {
        RecordingGap::default().note(&format!("empty {FILE}"), &error);
    }
}

/// A connection whose main database is `history.db`, so `BEGIN IMMEDIATE`
/// locks only it. The store is attached read-only for the attachment fences
/// rows check; a read-only attachment never takes its lock.
pub(crate) fn open_writer(store: &Path) -> StoreResult<Connection> {
    let conn = Connection::open(history_path(store))?;
    crate::instrumentation::record_connection_open();
    crate::instrumentation::observe_sqlite(&conn);
    conn.busy_timeout(BUSY)?;
    let mode: String =
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if mode != "wal" {
        return Err(crate::StoreError::InvalidData(format!(
            "{FILE} stayed in {mode} journal mode"
        )));
    }
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < VERSION {
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE; {} PRAGMA user_version = {VERSION}; COMMIT;",
            schema("")
        ))?;
    }
    conn.execute("ATTACH ?1 AS store", [read_only_uri(store)])?;
    Ok(conn)
}

/// A `file:` URI opening `path` read-only.
fn read_only_uri(path: &Path) -> String {
    let path = path
        .to_string_lossy()
        .replace('%', "%25")
        .replace('?', "%3f")
        .replace('#', "%23");
    format!("file:{path}?mode=ro")
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::history_path;
    use crate::sqlite::SqliteStore;
    use rusqlite::Connection;

    fn append(store: &SqliteStore, receipt: &str) -> Option<i64> {
        store.write_history("session history", |tx| {
            // The fence reads the store through the read-only attachment.
            let session: String =
                tx.query_row("SELECT id FROM store.agent_sessions", [], |row| row.get(0))?;
            tx.execute(
                "INSERT INTO session_history(session_id,receipt_key,observed_at,payload)
                 VALUES(?1,?2,0,'{}')",
                rusqlite::params![session, receipt],
            )?;
            Ok(tx.last_insert_rowid())
        })
    }

    #[test]
    fn history_is_written_while_the_store_write_lock_is_held() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("loopflow.db");
        let store = crate::test_fixture::open(&path).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());

        let mut holder = Connection::open(&path).unwrap();
        let held = holder
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        held.execute("UPDATE store_revisions SET revision=revision+1", [])
            .unwrap();

        let started = Instant::now();
        assert!(append(&store, "first").is_some());
        assert!(started.elapsed() < Duration::from_secs(1));
        // The attached store is read-only: history cannot write state.
        let wrote_state = store.write_history("state", |tx| {
            Ok(tx.execute("UPDATE store.store_revisions SET revision=revision+1", [])?)
        });
        assert_eq!(wrote_state, None);
        held.rollback().unwrap();

        let written: (String, String, i64) = Connection::open(history_path(&path))
            .unwrap()
            .query_row(
                "SELECT session_id,receipt_key,(SELECT revision FROM history_revision)
                 FROM session_history",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(written, ("session".into(), "first".into(), 1));
    }

    #[test]
    fn a_failed_history_write_is_a_gap_and_the_next_one_closes_it() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("loopflow.db");
        let store = crate::test_fixture::open(&path).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        assert!(append(&store, "first").is_some());

        // Another history writer holds the file past the busy wait.
        let mut other = Connection::open(history_path(&path)).unwrap();
        let held = other
            .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
            .unwrap();
        assert_eq!(append(&store, "missed"), None);
        let gap = store.history.gap().unwrap();
        assert!(gap.starts_with("history.db: "), "{gap}");
        // The gap belongs to the file, not the store: a store opened again
        // sees it open, so its writes skip the busy wait too.
        let again = crate::test_fixture::open(&path).unwrap();
        assert_eq!(again.history.gap(), Some(gap.clone()));
        assert_eq!(append(&again, "missed again"), None);
        // Another kind of write meeting the same lock is the same gap, so
        // lf.log warns once.
        assert_eq!(again.write_history("turn usage", |_| Ok(())), None);
        assert_eq!(again.history.gap(), Some(gap));
        // The store itself keeps answering writes while history is gapped.
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE store_revisions SET revision=revision+1", [])
            .unwrap();
        held.rollback().unwrap();

        assert!(append(&store, "after").is_some());
        assert_eq!(store.history.gap(), None);
    }

    #[test]
    fn the_store_reads_history_and_writes_while_a_history_writer_holds_its_lock() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("loopflow.db");
        let store = crate::test_fixture::open(&path).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        assert!(append(&store, "first").is_some());

        let mut other = Connection::open(history_path(&path)).unwrap();
        let held = other
            .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
            .unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute_batch(
            "BEGIN IMMEDIATE; UPDATE store_revisions SET revision=revision+1; COMMIT;",
        )
        .unwrap();
        let receipt: String = conn
            .query_row(
                "SELECT receipt_key FROM history.session_history h
                 JOIN agent_sessions s ON s.id=h.session_id",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(receipt, "first");
        drop(conn);
        held.rollback().unwrap();
    }
}
