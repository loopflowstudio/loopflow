//! Retention: the store keeps facts, and streams stay in files.
//!
//! This prune ends the runs and Session agents whose OS process is gone
//! without recording its exit, the liveness pass SQL readers rely on: a Task
//! reads moving while its carrier's pid is set. It moves the
//! history an earlier lf kept in `loopflow.db` into `history.db`, its turns
//! into `history.turns`, and forgets history older than [`HISTORY_DAYS`]. Each step
//! is one short transaction on its own connection, so live writers wait well
//! under their busy timeout. The Machine server runs it every hour.

use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::graph_process::RunOwner;
use crate::StoreResult;

/// Rows moved or forgotten per transaction.
const ROW_BATCH: i64 = 10_000;
/// Pages freed per incremental vacuum transaction: 40 MB of 4 KiB pages,
/// about a second of write lock on a laptop disk.
const VACUUM_PAGES: i64 = 10_000;
/// Every live writer waits this long; each prune transaction stays well under.
const BUSY_TIMEOUT: Duration = Duration::from_secs(15);
/// Session history is kept this long after it was observed. Turns and the
/// inputs in `loopflow.db` are kept.
pub const HISTORY_DAYS: i64 = 90;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionReport {
    /// Runs and Session agents ended because their OS process was gone.
    pub ended_owners: u64,
    /// Observations an earlier lf kept in `loopflow.db`, now in `history.db`.
    pub moved_observations: u64,
    /// Turns an earlier lf kept as three rows, now one row in `history.turns`.
    pub converted_turns: u64,
    /// Rows past [`HISTORY_DAYS`], and rows nothing reads, deleted.
    pub forgotten_history: u64,
    pub vacuumed: bool,
}

/// Prune `database` once, at `now` in seconds.
///
/// # Errors
/// When the store cannot be opened or a prune transaction fails.
pub fn prune(
    database: &Path,
    dead: &[(RunningOwner, i64)],
    now: i64,
) -> StoreResult<RetentionReport> {
    let conn = Connection::open(database)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // Nothing references the rows deleted here (see each predicate), and
    // per-row foreign-key probes would scan session_events for each delete.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    crate::history::attach(&conn, database);
    let history = crate::history::HistoryFile::beside(Some(database));
    let ended_owners = end_dead_owners(&conn, dead)?;
    let cutoff = now - HISTORY_DAYS * 86_400;
    let mut forgotten_history = forget_unread_rows(&conn, cutoff)?;
    let moved_observations = move_observations(&conn, &history)?;
    let converted_turns = super::turns::convert_turn_rows(database)?;
    forgotten_history += forget_old_history(&history, database, cutoff)?;
    Ok(RetentionReport {
        ended_owners,
        moved_observations,
        converted_turns,
        forgotten_history,
        vacuumed: reclaim(&conn)?,
    })
}

// ---------------------------------------------------------------------------
// History an earlier lf kept in loopflow.db

/// Observations in `loopflow.db` nothing reads: kinds no lf writes or reads
/// any more, departure-less attachment exits, the facts that became columns,
/// once carried there, and what an ordinary turn's row already says (its
/// first attempt starting and completing, and its completed turn). Over
/// alias `e`.
const UNREAD_OBSERVATION: &str = "e.kind='observed' AND (
    json_extract(e.payload,'$.source') IN ('task_update','codex_relay','codex_submission')
    OR json_extract(e.payload,'$.type') IN ('provider_launch','driver_exit','legacy_flow',
        'legacy_flow_review','agent_process_replaced','agent_process_ancestry_repaired',
        'interactive_opened')
    OR json_extract(e.payload,'$.evidence.type') IN ('provider_account_selected',
        'provider_session_observed')
    OR (json_extract(e.payload,'$.type')='attachment_exit'
        AND json_type(e.payload,'$.departure') IS NULL)
    OR instr(e.receipt_key,':provider-session:')>0
    OR (json_extract(e.payload,'$.evidence.attempt_key')='attempt-1'
        AND (json_extract(e.payload,'$.evidence.type')='provider_attempt_started'
            OR (json_extract(e.payload,'$.evidence.type')='provider_attempt_finished'
                AND json_extract(e.payload,'$.evidence.outcome')='completed')))
    OR (json_extract(e.payload,'$.evidence.type')='conversation'
        AND json_extract(e.payload,'$.evidence.event.type')='turn_completed'
        AND json_extract(e.payload,'$.evidence.event.status')='completed'))";

/// Carry the state facts an earlier lf wrote as observations onto their
/// owners, then delete what nothing reads in `loopflow.db`: unread
/// observations, history past `cutoff` (turn rows excepted: they convert),
/// and copies of the provider's last message past `cutoff`.
fn forget_unread_rows(conn: &Connection, cutoff: i64) -> StoreResult<u64> {
    let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    // The newest resume reference an input observed, as the migration did.
    tx.execute(
        "UPDATE session_events AS c SET
             provider_thread=json_extract(o.payload,'$.evidence.provider_session_id'),
             provider_account=json_extract(o.payload,'$.evidence.account_id')
         FROM (SELECT session_id,substr(receipt_key,1,instr(receipt_key,':provider-session:')-1) AS input,
                 payload,ROW_NUMBER() OVER (PARTITION BY session_id,
                     substr(receipt_key,1,instr(receipt_key,':provider-session:')-1) ORDER BY seq DESC) AS newest
               FROM session_events WHERE kind='observed' AND instr(receipt_key,':provider-session:')>0
                 AND json_valid(payload) AND json_extract(payload,'$.evidence.schema_version')=1
                 AND length(json_extract(payload,'$.evidence.provider_session_id'))>0) AS o
         WHERE c.kind='captured' AND c.session_id=o.session_id AND c.receipt_key=o.input
           AND o.newest=1",
        [],
    )?;
    tx.execute(
        "UPDATE agent_sessions AS s SET opened_at=MAX(COALESCE(s.opened_at,0),o.opened_at)
         FROM (SELECT session_id,MAX(json_extract(payload,'$.opened_at_ms')) AS opened_at
               FROM session_events WHERE kind='observed' AND json_valid(payload)
                 AND json_extract(payload,'$.type')='interactive_opened' GROUP BY session_id) AS o
         WHERE s.id=o.session_id AND s.interactive=1",
        [],
    )?;
    // Nothing reads which observation a Process's attachment exit was.
    tx.execute(
        "UPDATE processes SET attachment_exit_seq=NULL WHERE attachment_exit_seq IS NOT NULL",
        [],
    )?;
    tx.commit()?;
    let mut forgotten = 0;
    loop {
        let deleted = conn.execute(
            &format!(
                "DELETE FROM session_events WHERE seq IN (
                     SELECT e.seq FROM session_events e WHERE ({UNREAD_OBSERVATION})
                        OR (e.kind IN ('observed','output') AND e.observed_at < ?1)
                     LIMIT ?2)"
            ),
            params![cutoff, ROW_BATCH],
        )?;
        forgotten += deleted as u64;
        if deleted == 0 {
            return Ok(forgotten);
        }
    }
}

/// Move the observations left in `loopflow.db` into `history.db`, oldest
/// first: each batch is written there, then deleted here, so a pass cut short
/// moves the rest next time and a row already moved is not written twice.
fn move_observations(conn: &Connection, history: &crate::history::HistoryFile) -> StoreResult<u64> {
    let mut moved = 0;
    loop {
        let last: Option<i64> = conn.query_row(
            "SELECT MAX(seq) FROM (SELECT seq FROM session_events WHERE kind='observed'
                 ORDER BY seq LIMIT ?1)",
            [ROW_BATCH],
            |row| row.get(0),
        )?;
        let Some(last) = last else {
            return Ok(moved);
        };
        history
            .write("history move", |tx| {
                Ok(tx.execute(
                    "INSERT OR IGNORE INTO session_history(session_id,receipt_key,lf_process_id,
                         task_id,wave_id,captured_event,observed_at,payload)
                     SELECT session_id,receipt_key,lf_process_id,task_id,wave_id,captured_event,
                         observed_at,payload
                     FROM store.session_events WHERE kind='observed' AND seq<=?1 ORDER BY seq",
                    [last],
                )?)
            })
            .ok_or_else(|| crate::StoreError::InvalidData("history.db is unavailable".into()))?;
        moved += conn.execute(
            "DELETE FROM session_events WHERE kind='observed' AND seq<=?1",
            [last],
        )? as u64;
    }
}

/// Forget Session history observed before `cutoff`, then return its pages.
fn forget_old_history(
    history: &crate::history::HistoryFile,
    database: &Path,
    cutoff: i64,
) -> StoreResult<u64> {
    let mut forgotten = 0;
    loop {
        let deleted = history
            .write("history retention", |tx| {
                Ok(tx.execute(
                    "DELETE FROM session_history WHERE seq IN (
                         SELECT seq FROM session_history WHERE observed_at < ?1 LIMIT ?2)",
                    params![cutoff, ROW_BATCH],
                )?)
            })
            .ok_or_else(|| crate::StoreError::InvalidData("history.db is unavailable".into()))?;
        forgotten += deleted as u64;
        if deleted == 0 {
            break;
        }
    }
    let conn = Connection::open(crate::history::history_path(database))?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    reclaim(&conn)?;
    Ok(forgotten)
}

// ---------------------------------------------------------------------------
// Liveness: runs and agents whose OS process is gone

/// A record that runs as an OS process and has not ended: a Graph run or
/// step, a Task's carrier, or a
/// Session's agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningOwner {
    pub owner: Owner,
    /// The run's id, or the Session's current agent.
    pub id: String,
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
    /// When it started, where its record says.
    pub started_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Owner {
    Run(RunOwner),
    /// The Session whose current agent this is.
    SessionAgent {
        session: String,
    },
}

/// Every unended owner, each with the OS process it recorded. A Task's
/// carrier counts only while it records one: its end clears the pid alone.
const RUNNING_OWNERS: &str = "
    SELECT 'graph_run',lf_process_id,pid,os_started_at,started_at,NULL FROM graph_processes
        WHERE ended_at IS NULL
    UNION ALL SELECT 'graph_step',lf_process_id,pid,os_started_at,started_at,NULL
        FROM graph_process_steps WHERE ended_at IS NULL
    UNION ALL SELECT 'task_carrier',lf_process_id,run_pid,run_os_started_at,NULL,NULL
        FROM task_graphs WHERE lf_process_id IS NOT NULL AND run_pid IS NOT NULL
    UNION ALL SELECT 'session_agent',agent_process_id,agent_pid,agent_os_started_at,
        agent_started_at,id FROM agent_sessions
        WHERE agent_process_id IS NOT NULL AND agent_pid IS NOT NULL AND agent_ended_at IS NULL";

/// The owners to observe before pruning.
pub fn running_owners(database: &Path) -> StoreResult<Vec<RunningOwner>> {
    let conn = Connection::open(database)?;
    running_owners_in(&conn)
}

fn running_owners_in(conn: &Connection) -> StoreResult<Vec<RunningOwner>> {
    let mut statement = conn.prepare(RUNNING_OWNERS)?;
    let rows = statement.query_map([], |row| {
        let owner = match row.get::<_, String>(0)?.as_str() {
            "graph_run" => Owner::Run(RunOwner::GraphRun),
            "graph_step" => Owner::Run(RunOwner::GraphStep),
            "task_carrier" => Owner::Run(RunOwner::TaskCarrier),
            _ => Owner::SessionAgent {
                session: row.get(5)?,
            },
        };
        Ok(RunningOwner {
            owner,
            id: row.get(1)?,
            pid: row.get(2)?,
            os_started_at: row.get(3)?,
            started_at: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// End each owner observed dead, at its time, as nobody saw it exit: no exit
/// code, no error, its OS process cleared. One that changed since it was
/// observed (another start, or its own end) is left.
fn end_dead_owners(conn: &Connection, dead: &[(RunningOwner, i64)]) -> StoreResult<u64> {
    let mut ended = 0;
    for (observed, at) in dead {
        let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
        let unchanged = running_owners_in(&tx)?.contains(observed);
        if unchanged {
            match &observed.owner {
                // A carrier's own end clears its OS process and says nothing
                // more; a death is told apart by when it was found.
                Owner::Run(RunOwner::TaskCarrier) => {
                    tx.execute(
                        "UPDATE task_graphs SET run_pid=NULL,run_os_started_at=NULL,run_died_at=?2
                         WHERE lf_process_id=?1 AND run_pid IS NOT NULL",
                        params![observed.id, at],
                    )?;
                }
                Owner::Run(owner) => super::graph_inventory::finish_run_in(
                    &tx,
                    *owner,
                    &observed.id,
                    *at,
                    None,
                    None,
                )?,
                Owner::SessionAgent { session } => {
                    tx.execute(
                        "UPDATE agent_sessions SET agent_ended_at=?3,agent_pid=NULL,
                            agent_os_started_at=NULL,agent_endpoint=NULL,agent_spawn_state='exited'
                         WHERE id=?1 AND agent_process_id=?2 AND agent_ended_at IS NULL",
                        params![session, observed.id, at],
                    )?;
                    super::processes::complete_dead_headless_run_in(&tx, &observed.id, *at)?;
                }
            }
            ended += 1;
        }
        tx.commit()?;
    }
    Ok(ended)
}

/// Return freed pages to the filesystem. The first reclaim converts the store
/// to incremental auto-vacuum with one VACUUM; later passes free pages in place.
/// A busy store is left for the next pass.
fn reclaim(conn: &Connection) -> StoreResult<bool> {
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    let free: i64 = conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
    let mode: i64 = conn.query_row("PRAGMA auto_vacuum", [], |row| row.get(0))?;
    if mode == 2 {
        // Each step frees pages; a statement not stepped to its end frees
        // none. Chunks keep each write transaction short.
        let mut left = free + 1;
        loop {
            let free: i64 = conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
            if free == 0 || free >= left {
                return Ok(false);
            }
            left = free;
            let mut vacuum = conn.prepare(&format!("PRAGMA incremental_vacuum({VACUUM_PAGES})"))?;
            let mut steps = vacuum.query([])?;
            while steps.next()?.is_some() {}
        }
    }
    if free * 2 <= pages {
        return Ok(false);
    }
    match conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL; VACUUM;") {
        Ok(()) => Ok(true),
        Err(error) if is_busy(&error) => {
            tracing::info!(%error, "store reclaim deferred: the store is busy");
            Ok(false)
        }
        Err(error) => Err(error.into()),
    }
}

fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
}

#[cfg(test)]
mod observation_tests {
    use super::{prune, running_owners};

    #[test]
    fn a_finished_task_with_a_leftover_pid_owns_no_process() {
        let (_dir, store, task) = crate::test_fixture::store_with_task();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO task_graphs(task_id,graph,step,updated_at,run_pid,run_os_started_at)
                 VALUES(?1,'{}','end',1,123,456)",
                [task.as_str()],
            )
            .unwrap();
            assert!(super::running_owners_in(&conn).unwrap().is_empty());
        }
        assert!(store.recorded_lfs().unwrap().is_empty());
    }

    /// A Session agent that died unseen ends at the next pass, unless it
    /// changed between observation and pruning.
    #[test]
    fn a_dead_agent_ends_unless_it_changed_after_observation() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = crate::test_fixture::open(&path).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "UPDATE agent_sessions SET agent_process_id='agent',attachment_token='token',
                agent_started_at=1,agent_pid=123,agent_os_started_at=456,
                agent_spawn_state='spawn_requested' WHERE id='session'",
            [],
        )
        .unwrap();
        let saved = running_owners(&path).unwrap().remove(0);
        conn.execute(
            "UPDATE agent_sessions SET agent_pid=124 WHERE id='session'",
            [],
        )
        .unwrap();
        assert_eq!(prune(&path, &[(saved, 10)], 10).unwrap().ended_owners, 0);
        let current = running_owners(&path).unwrap().remove(0);
        assert_eq!(prune(&path, &[(current, 10)], 10).unwrap().ended_owners, 1);
        let ended: (Option<i64>, Option<u32>, Option<i32>) = conn
            .query_row(
                "SELECT agent_ended_at,agent_pid,agent_exit_code FROM agent_sessions WHERE id='session'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(ended, (Some(10), None, None));
        assert!(running_owners(&path).unwrap().is_empty());
    }
}

#[cfg(test)]
mod history_tests {
    use rusqlite::{params, Connection};

    use super::{prune, HISTORY_DAYS};

    /// An earlier lf's store: one Session whose history sits in
    /// `loopflow.db` beside its input, and a `history.db` with one row past
    /// retention and one within it.
    #[test]
    fn prune_moves_old_shape_history_out_and_forgets_only_aged_rows() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("loopflow.db");
        let store = crate::test_fixture::open(&path).unwrap();
        let input = crate::session_record::new_artifact_key();
        let session = store.test_session("conversation", &input);
        let captured = session.captured.unwrap();
        let now = 2_000_000_000;
        let recent = now - 86_400;
        let aged = now - (HISTORY_DAYS + 1) * 86_400;

        let conn = Connection::open(&path).unwrap();
        let observe = |kind: &str, receipt: String, at: i64, payload: serde_json::Value| {
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
                 VALUES('conversation',?1,?2,?3,?4,?5)",
                params![kind, receipt, at, payload.to_string(), captured],
            )
            .unwrap();
        };
        observe(
            "observed",
            format!("{input}:manifest.json"),
            recent,
            serde_json::json!({"source":"manifest.json"}),
        );
        observe(
            "observed",
            "attachment:kept:exit".into(),
            recent,
            serde_json::json!({"type":"attachment_exit","departure":{"kind":"handoff"}}),
        );
        observe(
            "observed",
            format!("{input}:store-gap:1:2"),
            aged,
            serde_json::json!({"source":"store-gap"}),
        );
        // Nothing reads these.
        observe(
            "observed",
            "task_update:x:accepted".into(),
            recent,
            serde_json::json!({"source":"task_update"}),
        );
        observe(
            "observed",
            "attachment:gone:exit".into(),
            recent,
            serde_json::json!({"type":"attachment_exit","outcome":"completed"}),
        );
        observe(
            "observed",
            format!("{input}:events.jsonl:1"),
            recent,
            serde_json::json!({"evidence":{"type":"provider_account_selected"}}),
        );
        // What an ordinary turn's row already says.
        observe(
            "observed",
            format!("{input}:events.jsonl:2"),
            recent,
            serde_json::json!({"evidence":{"type":"provider_attempt_started","attempt_key":"attempt-1"}}),
        );
        observe(
            "observed",
            format!("{input}:events.jsonl:3"),
            recent,
            serde_json::json!({"evidence":{"type":"conversation",
                "event":{"type":"turn_completed","status":"completed"}}}),
        );
        // State facts that became columns.
        observe(
            "observed",
            format!("{input}:provider-session:agent"),
            recent,
            serde_json::json!({
            "evidence":{"schema_version":1,"provider_session_id":"thread-1","account_id":"work"}}),
        );
        observe(
            "observed",
            format!("{input}:interactive_opened:a"),
            recent,
            serde_json::json!({"type":"interactive_opened","opened_at_ms":1_234}),
        );
        // A Task's only turn, as three rows: it converts, and the rows go.
        // Only the Task id matters here, not the Task.
        conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        for (kind, at) in [("started", recent), ("completed", recent + 5)] {
            conn.execute(
                "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,
                     task_id,observed_at,payload,captured_event)
                 VALUES('conversation','thread','turn',?1,?1,
                     'task_00000000000000000000000000000001',?2,'{}',?3)",
                params![kind, at, captured],
            )
            .unwrap();
        }
        // Copies of the provider's last message go by age, never moved.
        observe("output", "answer:old".into(), aged, serde_json::json!({}));
        observe("output", "answer:new".into(), recent, serde_json::json!({}));
        store
            .execute_history(
                "INSERT INTO session_history(session_id,receipt_key,observed_at,payload)
                 VALUES('conversation','history:old',?1,'{}'),
                       ('conversation','history:new',?2,'{}')",
                params![aged, recent],
            )
            .unwrap();

        let first = prune(&path, &[], now).unwrap();
        assert_eq!((first.moved_observations, first.converted_turns), (2, 1));

        let main: Vec<(String, String)> = conn
            .prepare("SELECT kind,receipt_key FROM session_events ORDER BY seq")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            main,
            [
                ("captured".to_string(), input.clone()),
                ("output".into(), "answer:new".into())
            ]
        );
        let state: (Option<String>, Option<String>, Option<i64>) = conn
            .query_row(
                "SELECT c.provider_thread,c.provider_account,s.opened_at
                 FROM session_events c JOIN agent_sessions s ON s.id=c.session_id
                 WHERE c.seq=?1",
                [captured],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            state,
            (Some("thread-1".into()), Some("work".into()), Some(1_234))
        );
        let history = || -> Vec<String> {
            let mut kept: Vec<String> = Connection::open(crate::history::history_path(&path))
                .unwrap()
                .prepare("SELECT receipt_key FROM session_history")
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            kept.sort();
            kept
        };
        let moved = history();
        let mut expected = vec![
            format!("{input}:manifest.json"),
            "attachment:kept:exit".to_string(),
            "history:new".to_string(),
        ];
        expected.sort();
        assert_eq!(moved, expected);

        // A second pass finds nothing left to move and keeps what it moved.
        let second = prune(&path, &[], now).unwrap();
        assert_eq!(
            (second.moved_observations, second.forgotten_history),
            (0, 0)
        );
        assert_eq!(history(), moved);
    }
}
