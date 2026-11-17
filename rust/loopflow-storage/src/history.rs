//! History a person reads later lives in `history.db` beside `loopflow.db`:
//! its own SQLite file with its own write lock, WAL and revision row. A history
//! write never takes the store's write lock, and one that fails is a
//! [`RecordingGap`] in `logs/lf.log`, never a failed turn.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::{RecordingGap, StoreResult};

const HISTORY_FILE: &str = "history.db";

/// The history file's schema, applied once per `user_version`. History is a
/// journal: a new version adds to it, and nothing migrates old rows. `{p}`
/// names the schema the objects go in: empty for the file itself, `history.`
/// for the empty stand-in a store attaches when it cannot reach the file.
const SCHEMA_VERSION: i64 = 1;
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS {p}session_history (
    seq INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    provider_thread TEXT,
    provider_turn TEXT,
    kind TEXT NOT NULL CHECK(kind IN ('started','usage','completed','observed')),
    receipt_key TEXT NOT NULL,
    lf_process_id TEXT,
    agent_process_id TEXT,
    task_id TEXT,
    wave_id TEXT,
    captured_event INTEGER,
    observed_at INTEGER NOT NULL,
    payload TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS {p}session_history_receipt ON session_history(
    session_id, COALESCE(provider_thread,''), COALESCE(provider_turn,''), kind, receipt_key);
CREATE INDEX IF NOT EXISTS {p}session_history_order ON session_history(session_id, seq);
CREATE INDEX IF NOT EXISTS {p}session_history_turn ON session_history(
    session_id, provider_thread, provider_turn, kind, observed_at);
CREATE INDEX IF NOT EXISTS {p}session_history_observation ON session_history(session_id, receipt_key)
    WHERE kind='observed';
CREATE INDEX IF NOT EXISTS {p}session_history_input ON session_history(captured_event)
    WHERE captured_event IS NOT NULL;
CREATE INDEX IF NOT EXISTS {p}session_history_outcome_window ON session_history(observed_at)
    WHERE kind='observed' AND substr(receipt_key,-14) IN (':manifest.json',':terminal.json');
CREATE INDEX IF NOT EXISTS {p}session_history_turn_window ON session_history(
    observed_at, kind, session_id, provider_thread, provider_turn)
    WHERE kind IN ('started','usage','completed');
CREATE INDEX IF NOT EXISTS {p}session_history_window ON session_history(observed_at);
CREATE INDEX IF NOT EXISTS {p}session_history_process ON session_history(lf_process_id, session_id)
    WHERE lf_process_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS {p}session_history_task ON session_history(task_id)
    WHERE task_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS {p}session_history_transitions ON session_history(session_id, seq)
    WHERE kind='observed' AND receipt_key GLOB 'provider-transition:*';

CREATE TABLE IF NOT EXISTS {p}history_revision (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    revision INTEGER NOT NULL
);
INSERT OR IGNORE INTO {p}history_revision(id, revision) VALUES(1, 0);
CREATE TRIGGER IF NOT EXISTS {p}history_revision_session_insert AFTER INSERT ON session_history
BEGIN UPDATE history_revision SET revision = revision + 1; END;
CREATE TRIGGER IF NOT EXISTS {p}history_revision_session_delete AFTER DELETE ON session_history
BEGIN UPDATE history_revision SET revision = revision + 1; END;
";

fn schema(prefix: &str) -> String {
    SCHEMA.replace("{p}", prefix)
}

/// One row of `history.session_history`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Row {
    pub session_id: String,
    pub provider_thread: Option<String>,
    pub provider_turn: Option<String>,
    pub kind: &'static str,
    pub receipt_key: String,
    pub lf_process_id: Option<String>,
    pub agent_process_id: Option<String>,
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
            kind: "observed",
            receipt_key,
            captured_event,
            observed_at,
            payload,
            ..Self::default()
        }
    }
}

/// Append `row` and return its sequence; a receipt already kept answers with
/// its own, and one kept with another payload is conflicting evidence.
pub(crate) fn append_in(tx: &Connection, row: &Row) -> StoreResult<i64> {
    let kept: Option<(i64, String)> = tx
        .query_row(
            "SELECT seq,payload FROM session_history WHERE session_id=?1
             AND COALESCE(provider_thread,'')=COALESCE(?2,'') AND COALESCE(provider_turn,'')=COALESCE(?3,'')
             AND kind=?4 AND receipt_key=?5",
            rusqlite::params![
                row.session_id,
                row.provider_thread,
                row.provider_turn,
                row.kind,
                row.receipt_key
            ],
            |found| Ok((found.get(0)?, found.get(1)?)),
        )
        .optional()?;
    if let Some((seq, payload)) = kept {
        if payload == row.payload {
            return Ok(seq);
        }
        return Err(crate::StoreError::InvalidData(format!(
            "conflicting {} {} receipt for Session {}",
            row.kind, row.receipt_key, row.session_id
        )));
    }
    tx.execute(
        "INSERT INTO session_history(session_id,provider_thread,provider_turn,kind,receipt_key,
            lf_process_id,agent_process_id,task_id,wave_id,captured_event,observed_at,payload)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        rusqlite::params![
            row.session_id,
            row.provider_thread,
            row.provider_turn,
            row.kind,
            row.receipt_key,
            row.lf_process_id,
            row.agent_process_id,
            row.task_id,
            row.wave_id,
            row.captured_event,
            row.observed_at,
            row.payload
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

/// A history write waits this long for another history writer, then becomes a
/// gap: a turn boundary does not queue behind history.
#[cfg(not(test))]
const BUSY: Duration = Duration::from_secs(2);
#[cfg(test)]
const BUSY: Duration = Duration::from_millis(50);

/// `history.db` in the directory of the store at `store`.
pub fn history_path(store: &Path) -> PathBuf {
    store.with_file_name(HISTORY_FILE)
}

/// The history writer of one store connection: opened at its first write, and
/// reopened after a failure. `store` is the store beside which history lives;
/// a store without a file keeps none.
#[derive(Debug, Default)]
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
    /// The writer of the history kept beside the store at `store`.
    pub(crate) fn beside(store: &Path) -> Self {
        Self {
            store: Some(store.to_path_buf()),
            state: Mutex::default(),
        }
    }

    /// Run `write` in one transaction on the history file, with the store
    /// attached read-only as `store`. Returns what it committed; a failure is
    /// noted as a recording gap for `what`.
    pub(crate) fn write<T>(
        &self,
        what: &str,
        write: impl FnOnce(&Transaction<'_>) -> StoreResult<T>,
    ) -> Option<T> {
        let mut state = self.state.lock().expect("history mutex poisoned");
        let Some(store) = &self.store else {
            state
                .gap
                .note(what, &"the store has no file beside which to keep history");
            return None;
        };
        let result = commit(&mut state.conn, store, write);
        match result {
            Ok(value) => {
                state.gap.recovered();
                Some(value)
            }
            Err(error) => {
                // A connection that failed mid-write is reopened next time.
                state.conn = None;
                state.gap.note(what, &error);
                None
            }
        }
    }

    #[cfg(test)]
    fn gap_open(&self) -> bool {
        self.state.lock().unwrap().gap.span().is_some()
    }
}

fn commit<T>(
    conn: &mut Option<Connection>,
    store: &Path,
    write: impl FnOnce(&Transaction<'_>) -> StoreResult<T>,
) -> StoreResult<T> {
    if conn.is_none() {
        *conn = Some(open_writer(store)?);
    }
    let conn = conn.as_mut().expect("history connection was just opened");
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let value = write(&tx)?;
    tx.commit()?;
    Ok(value)
}

/// Attach the history file of the store at `store` to `conn` read-only as
/// `history`, so readers name `history.session_history`, making the file and
/// its schema first when it is missing. History a store cannot reach is a
/// recording gap, never a failed open: readers then see an empty history.
pub(crate) fn attach(conn: &Connection, store: &Path) {
    let path = history_path(store);
    let attached = (|| -> StoreResult<()> {
        if !path.exists() {
            open_writer(store)?;
        }
        conn.execute("ATTACH ?1 AS history", [read_only_uri(&path)])?;
        Ok(())
    })();
    if let Err(error) = attached {
        RecordingGap::default().note("history file", &error);
        attach_empty(conn);
    }
}

/// An empty in-memory history, for a store without a reachable history file.
pub(crate) fn attach_empty(conn: &Connection) {
    // A read-only store connection is `query_only`; its empty history is not
    // a write to anything the person keeps.
    let attached = (|| -> StoreResult<()> {
        let query_only: bool = conn.pragma_query_value(None, "query_only", |row| row.get(0))?;
        conn.execute_batch(&format!(
            "PRAGMA query_only=OFF; ATTACH ':memory:' AS history; {} PRAGMA query_only={};",
            schema("history."),
            if query_only { "ON" } else { "OFF" }
        ))?;
        Ok(())
    })();
    if let Err(error) = attached {
        RecordingGap::default().note("empty history", &error);
    }
}

/// A connection whose main database is `history.db`, so `BEGIN IMMEDIATE`
/// locks only history. The store is attached read-only for the attachment
/// fences history rows check; a read-only attachment never takes its lock.
pub(crate) fn open_writer(store: &Path) -> StoreResult<Connection> {
    let conn = Connection::open(history_path(store))?;
    crate::instrumentation::observe_sqlite(&conn);
    conn.busy_timeout(BUSY)?;
    let mode: String =
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if mode != "wal" {
        return Err(crate::StoreError::InvalidData(format!(
            "history file stayed in {mode} journal mode"
        )));
    }
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < SCHEMA_VERSION {
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE; {} PRAGMA user_version = {SCHEMA_VERSION}; COMMIT;",
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
                "INSERT INTO session_history(session_id,kind,receipt_key,observed_at,payload)
                 VALUES(?1,'observed',?2,0,'{}')",
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
        assert!(store.history.gap_open());
        // The store itself keeps answering writes while history is gapped.
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE store_revisions SET revision=revision+1", [])
            .unwrap();
        held.rollback().unwrap();

        assert!(append(&store, "after").is_some());
        assert!(!store.history.gap_open());
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
