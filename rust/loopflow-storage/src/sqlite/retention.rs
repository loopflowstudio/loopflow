//! Retention: the store keeps facts, and streams stay in files.
//!
//! This prune closes Processes whose OS process is gone without recording its exit,
//! then forgets finished Processes nothing needs, as exit does. It moves the
//! history an earlier lf kept in `loopflow.db` into `history.db`, its turns
//! into `history.turns`, and forgets history older than [`HISTORY_DAYS`]. Each step
//! is one short transaction on its own connection, so live writers wait well
//! under their busy timeout. The Machine server runs it every hour.

use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::StoreResult;

const PROCESS_BATCH: i64 = 10_000;
/// Processes given their Task per transaction.
const TASK_BATCH: i64 = 5_000;
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
    pub closed_processes: u64,
    pub forgotten_processes: u64,
    /// Processes written without a Task (by an earlier lf) that now record one.
    pub assigned_processes: u64,
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
    dead: &[(crate::process::ProcessEntry, Option<String>, i64)],
    now: i64,
) -> StoreResult<RetentionReport> {
    let conn = Connection::open(database)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // Nothing references the rows deleted here (see each predicate), and
    // per-row foreign-key probes would scan session_events for each delete.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    crate::history::attach(&conn, database);
    let history = crate::history::HistoryFile::beside(Some(database));
    let closed_processes = close_dead_processes(&conn, dead)?;
    let cutoff = now - HISTORY_DAYS * 86_400;
    let mut forgotten_history = forget_unread_rows(&conn, cutoff)?;
    let moved_observations = move_observations(&conn, &history)?;
    let converted_turns = super::turns::convert_turn_rows(database)?;
    forgotten_history += forget_old_history(&history, database, cutoff)?;
    let forgotten_processes = forget_processes(&conn)?;
    Ok(RetentionReport {
        closed_processes,
        forgotten_processes,
        assigned_processes: assign_process_tasks(&conn)?,
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

/// Saved identities and locators for observation before pruning.
pub fn unfinished_processes(
    database: &Path,
) -> StoreResult<Vec<(crate::process::ProcessEntry, Option<String>)>> {
    let conn = Connection::open(database)?;
    let unfinished = conn
        .prepare(&format!(
            "{} WHERE e.completed_at IS NULL",
            process_receipt_select()
        ))?
        .query_map([], process_receipt_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(unfinished)
}

fn process_receipt_select() -> String {
    super::processes::PROCESS_SELECT
        .replace(" FROM processes e", ",e.receipt_locator FROM processes e")
}

fn process_receipt_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<(crate::process::ProcessEntry, Option<String>)> {
    Ok((
        super::processes::read_process(row)?,
        row.get("receipt_locator")?,
    ))
}

fn close_dead_processes(
    conn: &Connection,
    dead: &[(crate::process::ProcessEntry, Option<String>, i64)],
) -> StoreResult<u64> {
    let mut closed = 0;
    for (process, locator, at) in dead {
        let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
        let current = tx
            .query_row(
                &format!("{} WHERE e.id=?1", process_receipt_select()),
                [process.id()],
                process_receipt_row,
            )
            .optional()?;
        if !current
            .as_ref()
            .is_some_and(|(saved, saved_locator)| saved == process && saved_locator == locator)
        {
            continue;
        }
        let closed_here = tx.execute(
            "UPDATE processes SET completed_at=?2,
                 spawn_state=CASE WHEN kind!='agent' THEN spawn_state
                     WHEN spawn_state='reserved' AND pid IS NULL THEN 'spawn_failed' ELSE 'exited' END,
                 endpoint=NULL
             WHERE id=?1 AND completed_at IS NULL AND pid IS ?3 AND os_started_at IS ?4",
            params![process.id(), at, process.pid(), process.os_started_at()],
        )?;
        if closed_here > 0 {
            super::processes::complete_dead_headless_run_in(&tx, process.id(), *at)?;
        }
        tx.commit()?;
        closed += closed_here as u64;
    }
    Ok(closed)
}

/// Finished Processes nothing needs ([`super::processes::FORGETTABLE`]): those
/// crash-closed above, and those a finished parent left behind when it exited
/// before them. Exit forgets the rest as it happens.
fn forget_processes(conn: &Connection) -> StoreResult<u64> {
    let forgettable = super::processes::FORGETTABLE;
    let mut forgotten = 0;
    loop {
        let deleted = conn.execute(
            &format!(
                "DELETE FROM processes WHERE id IN (
                     SELECT p.id FROM processes p WHERE {forgettable} LIMIT ?1)"
            ),
            [PROCESS_BATCH],
        )?;
        forgotten += deleted as u64;
        // A parent becomes forgettable once its last child goes.
        if deleted == 0 {
            return Ok(forgotten);
        }
    }
}

/// Give each Process written without a Task the one it belongs to, oldest
/// first, so a parent or caller resolves before the rows that inherit from it.
/// The same rules as an insert, plus what an earlier lf recorded instead: the
/// Task a Graph names, and the Task of a Session the Process started.
/// `process_task_backfill` remembers how far the pass has read, so a row with
/// no Task is examined once.
fn assign_process_tasks(conn: &Connection) -> StoreResult<u64> {
    let lf = super::task_work::process_task(
        "p.parent_lf_process_id",
        "p.cwd",
        "p.caller_session_id",
        "p.caller_agent_process_id",
    );
    let task = format!(
        "SELECT CASE WHEN p.kind='agent'
            THEN (SELECT {} FROM agent_sessions s WHERE s.id=p.agent_session_id)
            ELSE coalesce((SELECT task_id FROM graph_processes WHERE lf_process_id=p.id),
                (SELECT a.task_id FROM session_events se INDEXED BY session_events_process
                    JOIN agent_sessions a ON a.id=se.session_id
                    WHERE se.lf_process_id=p.id AND se.kind IN ('captured','started')
                    AND {member} AND a.task_id IS NOT NULL LIMIT 1),
                {lf}) END AS task_id
         FROM processes p WHERE p.rowid=?1 AND p.task_id IS NULL",
        super::task_work::agent_task("s"),
        member = super::task_work::counts_for_task("a"),
    );
    let mut assigned = 0;
    loop {
        let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
        let rows = tx
            .prepare(
                "SELECT rowid FROM processes
                 WHERE rowid>(SELECT after_rowid FROM process_task_backfill) ORDER BY rowid LIMIT ?1",
            )?
            .query_map([TASK_BATCH], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let Some(&last) = rows.last() else {
            return Ok(assigned);
        };
        let mut assign = tx.prepare(&format!(
            "UPDATE processes SET task_id=found.task_id FROM ({task}) found
             WHERE processes.rowid=?1 AND found.task_id IS NOT NULL"
        ))?;
        for row in &rows {
            assigned += assign.execute([row])? as u64;
        }
        drop(assign);
        tx.execute("UPDATE process_task_backfill SET after_rowid=?1", [last])?;
        tx.commit()?;
    }
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
mod tests {
    use rusqlite::Connection;

    use super::prune;

    #[test]
    fn prune_forgets_unneeded_processes() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        crate::test_fixture::open(&path).unwrap();

        let now = time::OffsetDateTime::now_utc().unix_timestamp() + 2 * 86_400;
        let conn = Connection::open(&path).unwrap();
        let id = |n: u8| format!("00000000-0000-4000-8000-0000000000{n:02}");
        let process = |n: u8, parent: Option<u8>, finished: bool, via_agent: bool| {
            conn.execute(
                "INSERT INTO processes(id,trace_id,parent_lf_process_id,started_at,completed_at,outcome,via_agent)
                 VALUES(?1,?1,?2,?3,?4,?5,?6)",
                rusqlite::params![
                    id(n),
                    parent.map(id),
                    now,
                    finished.then_some(now),
                    finished.then_some("succeeded"),
                    via_agent
                ],
            )
            .unwrap();
        };
        // Finished long ago or just now, nothing needs these three.
        process(1, None, true, false);
        process(7, None, true, false);
        process(8, Some(7), true, false);
        // A running child holds its parent; a running parent holds its child.
        process(2, None, true, false);
        process(3, Some(2), false, false);
        process(4, Some(3), true, false);
        process(5, None, true, true);

        assert_eq!(prune(&path, &[], now).unwrap().forgotten_processes, 3);

        let left: Vec<String> = conn
            .prepare("SELECT id FROM processes ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(left, [id(2), id(3), id(4), id(5)]);
        let violations: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);
    }

    #[test]
    fn prune_gives_processes_an_earlier_lf_wrote_their_task_once() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = crate::test_fixture::open(&path).unwrap();
        let repository = crate::test_fixture::repository(&store, "/repo");
        let task = |title: &str| {
            store
                .create_task(&crate::work::task::NewTask {
                    id: loopflow_model::durable::TaskId::new(),
                    repository_id: repository.clone(),
                    project_id: None,
                    title: title.into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap()
                .id
        };
        let (checked_out, named) = (task("Checked out"), task("Named"));
        drop(store);
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/repo.task')",
            [checked_out.as_str()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id)
             VALUES('bound','Bound','human',1,0,'/elsewhere',?1)",
            [named.as_str()],
        )
        .unwrap();
        // Rows as released lf writes them: no Task, running, so nothing forgets them.
        let id = |n: u8| format!("00000000-0000-4000-8000-0000000000{n:02}");
        let insert = |n: u8, kind: &str, cwd: &str, parent: Option<u8>, caller: Option<u8>| {
            conn.execute(
                "INSERT INTO processes(id,trace_id,kind,cwd,parent_lf_process_id,caller_agent_process_id,
                    agent_session_id,started_at)
                 VALUES(?1,?1,?2,?3,?4,?5,CASE WHEN ?2='agent' THEN 'bound' END,?6)",
                rusqlite::params![id(n), kind, cwd, parent.map(id), caller.map(id), now],
            )
            .unwrap();
        };
        insert(1, "lf", "/repo.task/src", None, None);
        insert(2, "lf", "/tmp", Some(1), None);
        insert(3, "agent", "/elsewhere", Some(2), None);
        insert(4, "lf", "/tmp", Some(1), Some(3));
        insert(5, "lf", "/elsewhere", None, None);
        insert(6, "lf", "/repo.task", None, Some(3));
        let tasks = || -> Vec<Option<String>> {
            conn.prepare("SELECT task_id FROM processes ORDER BY id")
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(prune(&path, &[], now).unwrap().assigned_processes, 5);
        let (checked_out, named) = (Some(checked_out.to_string()), Some(named.to_string()));
        // The checkout, the parent, the agent's Session, the calling agent over
        // the parent; a row with no Task stays without one; and a checkout
        // over the calling agent.
        assert_eq!(
            tasks(),
            [
                checked_out.clone(),
                checked_out.clone(),
                named.clone(),
                named,
                None,
                checked_out
            ]
        );
        assert_eq!(prune(&path, &[], now).unwrap().assigned_processes, 0);
    }
}

#[cfg(test)]
mod observation_tests {
    use super::{prune, unfinished_processes};

    #[test]
    fn pruning_does_not_close_a_process_changed_after_observation() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let _store = crate::test_fixture::open(&path).unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        let id = "00000000-0000-4000-8000-000000000001";
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at,via_agent) VALUES(?1,?1,1,1)",
            [id],
        )
        .unwrap();
        let (saved, locator) = unfinished_processes(&path).unwrap().remove(0);
        conn.execute(
            "UPDATE processes SET pid=123,os_started_at=456 WHERE id=?1",
            [id],
        )
        .unwrap();
        assert_eq!(
            prune(&path, &[(saved, locator, 10)], 10)
                .unwrap()
                .closed_processes,
            0
        );
        assert!(conn
            .query_row(
                "SELECT completed_at IS NULL FROM processes WHERE id=?1",
                [id],
                |row| row.get::<_, bool>(0)
            )
            .unwrap());
        let (current, locator) = unfinished_processes(&path).unwrap().remove(0);
        assert_eq!(
            prune(&path, &[(current, locator, 10)], 10)
                .unwrap()
                .closed_processes,
            1
        );
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
