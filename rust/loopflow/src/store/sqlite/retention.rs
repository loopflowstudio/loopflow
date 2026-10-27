//! Retention: the store keeps facts, and streams stay in files.
//!
//! The recorder retains only the stream events history reads, and one usage
//! row per stream or native turn. Captures it recorded before those rules kept
//! every provider line and usage observation; this prune brings them to the
//! same shape and records the silences their lines implied. It closes Processes
//! whose OS process is gone without recording its exit, then forgets finished
//! Processes nothing needs, as exit does.
//! Each step is one short transaction on its own connection, so live writers
//! wait well under their busy timeout.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::journal::{OsProcess, ProcessIdentityEvidence};
use crate::session_record::{
    late_usage_key, merge_native_usage, merge_usage_observation, native_usage_key, usage_identity,
    SILENCE_SECONDS,
};
use crate::store::{StoreError, StoreResult};

/// Captures younger than this may still be written by an earlier lf.
const CAPTURE_SETTLE_SECONDS: i64 = 24 * 3600;
const PROCESS_BATCH: i64 = 10_000;
/// Every live writer waits this long; each prune transaction stays well under.
const BUSY_TIMEOUT: Duration = Duration::from_secs(15);

/// Stream events history keeps; must match `session_record::retained_in_history`.
const RETAINED: &str = "CASE WHEN json_valid(payload) THEN
        json_extract(payload,'$.evidence.type') IN ('provider_attempt_started',
            'provider_attempt_finished','provider_account_selected','provider_session_observed','usage')
        OR (json_extract(payload,'$.evidence.type')='conversation'
            AND json_extract(payload,'$.evidence.event.type') IN ('turn_completed','error'))
    ELSE 0 END";

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct RetentionReport {
    pub captures: u64,
    pub stream_rows: u64,
    pub usage_rows: u64,
    pub closed_processes: u64,
    pub processes: u64,
    pub vacuumed: bool,
}

/// Prune `database` once. A cursor file beside it holds the last settled
/// capture already visited, so a pass after the first reads only new captures
/// and a draft-build copy keeps its own.
///
/// # Errors
/// When the store cannot be opened or a prune transaction fails.
pub(crate) fn prune(database: &Path, now: i64) -> StoreResult<RetentionReport> {
    let conn = Connection::open(database)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // Nothing references the rows deleted here (see each predicate), and
    // per-row foreign-key probes would scan session_events for each delete.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let mut report = RetentionReport::default();
    prune_captures(
        &conn,
        &Cursor::beside(database, "retention"),
        now,
        &mut report,
    )?;
    report.usage_rows = compact_usage(&conn, &Cursor::beside(database, "usage"))?;
    let home = database.parent().unwrap_or(Path::new("."));
    report.closed_processes = close_dead_processes(&conn, home, now)?;
    report.processes = forget_processes(&conn)?;
    report.vacuumed = reclaim(&conn)?;
    Ok(report)
}

/// The last row a pass visited, in a file beside the database, so a later pass
/// reads only newer rows and a draft-build copy keeps its own.
struct Cursor(std::path::PathBuf);

impl Cursor {
    fn beside(database: &Path, name: &str) -> Self {
        let mut path = database.as_os_str().to_owned();
        path.push(format!(".{name}-cursor"));
        Self(path.into())
    }

    fn read(&self) -> i64 {
        std::fs::read_to_string(&self.0)
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(0)
    }

    fn write(&self, seq: i64) -> StoreResult<()> {
        std::fs::write(&self.0, seq.to_string())
            .map_err(|error| StoreError::InvalidData(error.to_string()))
    }
}

fn prune_captures(
    conn: &Connection,
    cursor: &Cursor,
    now: i64,
    report: &mut RetentionReport,
) -> StoreResult<()> {
    let mut after = cursor.read();
    loop {
        let next: Option<(i64, String, String, i64)> = conn
            .query_row(
                "SELECT seq,session_id,receipt_key,observed_at FROM session_events
                 WHERE kind='captured' AND seq>?1 ORDER BY seq LIMIT 1",
                [after],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let Some((captured, session, input, observed_at)) = next else {
            return Ok(());
        };
        if observed_at > now - CAPTURE_SETTLE_SECONDS {
            return Ok(());
        }
        let deleted = prune_capture(conn, captured, &session, &input)?;
        if deleted > 0 {
            report.captures += 1;
            report.stream_rows += deleted;
        }
        after = captured;
        cursor.write(after)?;
    }
}

/// Bring one capture to the recorder's shape. Only a capture with stream rows
/// the recorder no longer keeps is touched: its silences come from every line
/// it retained, which a capture recorded under the current rule never has.
fn prune_capture(conn: &Connection, captured: i64, session: &str, input: &str) -> StoreResult<u64> {
    let stream = "session_id=?3 AND captured_event=?2 AND kind='observed'
        AND substr(receipt_key,1,length(?1)+14)=?1||':events.jsonl:'";
    let tx = conn.unchecked_transaction()?;
    let legacy: bool = tx.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM session_events WHERE {stream} AND NOT ({RETAINED}))"),
        params![input, captured, session],
        |row| row.get(0),
    )?;
    if !legacy {
        return Ok(0);
    }
    tx.execute(
        &format!(
            "INSERT OR IGNORE INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event)
             SELECT session_id,'observed',?1||':silence:'||ordinal,task_id,wave_id,observed_at,
                 json_object('input_id',?1,'source','silence:'||ordinal,
                     'evidence',json_object('since',since,'observed_at',observed_at)),?2
             FROM (SELECT session_id,task_id,wave_id,observed_at,
                       CAST(substr(receipt_key,length(?1)+15) AS INTEGER) AS ordinal,
                       LAG(observed_at) OVER (ORDER BY CAST(substr(receipt_key,length(?1)+15) AS INTEGER),seq) AS since
                   FROM session_events WHERE {stream})
             WHERE observed_at-since>{SILENCE_SECONDS}"
        ),
        params![input, captured, session],
    )?;
    let deleted = tx.execute(
        &format!("DELETE FROM session_events WHERE {stream} AND NOT ({RETAINED})"),
        params![input, captured, session],
    )?;
    // Replay reads the prompt from the capture's own manifest file.
    tx.execute(
        "UPDATE session_events SET payload=json_set(payload,'$.evidence.exec',json('null'))
         WHERE session_id=?3 AND captured_event=?2 AND kind='observed' AND receipt_key=?1||':manifest.json'
           AND json_valid(payload) AND json_extract(payload,'$.evidence.exec') IS NOT NULL",
        params![input, captured, session],
    )?;
    tx.commit()?;
    Ok(deleted as u64)
}

/// Merge usage recorded as one row per observation into one row per stream or
/// native turn, as the recorder now writes it. Returns the rows removed.
fn compact_usage(conn: &Connection, cursor: &Cursor) -> StoreResult<u64> {
    let after = cursor.read();
    let last: Option<i64> =
        conn.query_row("SELECT max(seq) FROM session_events", [], |row| row.get(0))?;
    let Some(last) = last.filter(|last| *last > after) else {
        return Ok(0);
    };
    let turns = conn
        .prepare(
            "SELECT DISTINCT session_id,provider_thread,provider_turn FROM session_events
             WHERE seq>?1 AND seq<=?2 AND kind='usage'",
        )?
        .query_map([after, last], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let inputs = conn
        .prepare(
            "SELECT DISTINCT e.session_id,c.receipt_key FROM session_events e
             JOIN session_events c ON c.seq=e.captured_event AND c.kind='captured'
             WHERE e.seq>?1 AND e.seq<=?2 AND e.kind='observed' AND instr(e.receipt_key,':events.jsonl:')>0
               AND json_valid(e.payload) AND json_extract(e.payload,'$.evidence.type')='usage'",
        )?
        .query_map([after, last], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut removed = 0;
    for (session, thread, turn) in turns {
        removed += compact_native_turn(conn, &session, &thread, &turn)?;
    }
    for (session, input) in inputs {
        removed += compact_input_usage(conn, &session, &input)?;
    }
    cursor.write(last)?;
    Ok(removed)
}

/// One row per native usage key in the turn, at its latest receipt's position.
fn compact_native_turn(
    conn: &Connection,
    session: &str,
    thread: &str,
    turn: &str,
) -> StoreResult<u64> {
    let tx = conn.unchecked_transaction()?;
    let rows = tx
        .prepare(
            "SELECT seq,receipt_key,payload FROM session_events
             WHERE session_id=?1 AND provider_thread=?2 AND provider_turn=?3 AND kind='usage' ORDER BY seq",
        )?
        .query_map([session, thread, turn], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let starts = tx
        .prepare(
            "SELECT seq FROM session_events WHERE session_id=?1 AND provider_thread=?2
             AND provider_turn!=?3 AND kind='started' ORDER BY seq",
        )?
        .query_map([session, thread, turn], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    // As the recorder keys them: a receipt that arrives after the thread's
    // next turn began keeps a row of its own.
    let mut turn_rows = BTreeMap::<String, i64>::new();
    let mut keys = BTreeMap::<String, Vec<(i64, String, serde_json::Value)>>::new();
    for (seq, receipt, payload) in rows {
        // An unreadable receipt stays as it was recorded.
        let Ok(payload) = serde_json::from_str::<serde_json::Value>(&payload) else {
            continue;
        };
        let base = native_usage_key(&payload);
        let started = starts.iter().rev().find(|start| **start < seq).copied();
        let key = match (turn_rows.get(&base), started) {
            (Some(last), Some(started)) if started > *last => late_usage_key(&base, started),
            _ => {
                turn_rows.insert(base.clone(), seq);
                base
            }
        };
        keys.entry(key).or_default().push((seq, receipt, payload));
    }
    let mut removed = 0;
    for (key, receipts) in keys {
        if let [(_, receipt, _)] = receipts.as_slice() {
            if *receipt == key {
                continue;
            }
        }
        let kept = receipts.last().expect("a key has a receipt").0;
        let merged = receipts
            .iter()
            .map(|(_, _, payload)| payload.clone())
            .reduce(|saved, later| merge_native_usage(&saved, &later))
            .expect("a key has a receipt");
        for (seq, ..) in &receipts[..receipts.len() - 1] {
            removed += tx.execute("DELETE FROM session_events WHERE seq=?1", [seq])? as u64;
        }
        tx.execute(
            "UPDATE session_events SET receipt_key=?2,payload=?3 WHERE seq=?1",
            params![kept, key, serde_json::to_string(&merged)?],
        )?;
    }
    tx.commit()?;
    Ok(removed)
}

/// One row per usage stream and turn of a captured input, at its first observation.
fn compact_input_usage(conn: &Connection, session: &str, input: &str) -> StoreResult<u64> {
    let tx = conn.unchecked_transaction()?;
    let rows = tx
        .prepare(
            "SELECT seq,payload FROM session_events
             WHERE session_id=?1 AND kind='observed'
               AND receipt_key>=?2||':events.jsonl:' AND receipt_key<?2||':events.jsonl;'
               AND json_valid(payload) AND json_extract(payload,'$.evidence.type')='usage'",
        )?
        .query_map([session, input], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut streams = BTreeMap::<String, Vec<(u64, i64, serde_json::Value)>>::new();
    for (seq, payload) in rows {
        let payload: serde_json::Value = serde_json::from_str(&payload)?;
        let (Some(identity), Some(position)) = (
            usage_identity(&payload["evidence"]),
            payload["source"]
                .as_str()
                .and_then(|source| source.strip_prefix("events.jsonl:"))
                .and_then(|position| position.parse().ok()),
        ) else {
            continue;
        };
        streams
            .entry(identity)
            .or_default()
            .push((position, seq, payload));
    }
    let mut removed = 0;
    for mut observations in streams.into_values().filter(|rows| rows.len() > 1) {
        observations.sort_by_key(|(position, ..)| *position);
        // A stream with an unreadable observation stays as it was recorded.
        let merged = observations[1..]
            .iter()
            .try_fold(observations[0].2.clone(), |merged, (.., later)| {
                merge_usage_observation(&merged, later)
            });
        let Some(merged) = merged else {
            continue;
        };
        for (_, seq, _) in &observations[1..] {
            removed += tx.execute("DELETE FROM session_events WHERE seq=?1", [seq])? as u64;
        }
        tx.execute(
            "UPDATE session_events SET payload=?2 WHERE seq=?1",
            params![observations[0].1, serde_json::to_string(&merged)?],
        )?;
    }
    tx.commit()?;
    Ok(removed)
}

/// Close unfinished Processes whose OS process is gone: a restart, SIGKILL, a
/// crash or a closed terminal ends lf without its exit path. As when a dead
/// agent is settled, `completed_at` records when it was found dead and the
/// outcome stays unknown. A machine restart bounds the exit by the boot time.
fn close_dead_processes(conn: &Connection, home: &Path, now: i64) -> StoreResult<u64> {
    let columns = super::processes::PROCESS_SELECT
        .strip_suffix(" FROM processes e")
        .expect("Process select ends with its table");
    let unfinished = conn
        .prepare(&format!(
            "{columns},e.receipt_locator FROM processes e WHERE e.completed_at IS NULL"
        ))?
        .query_map([], |row| {
            Ok((
                super::processes::read_process(row)?,
                row.get::<_, Option<String>>("receipt_locator")?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let booted_at = crate::journal::machine_booted_at();
    let mut closed = 0;
    for (process, locator) in unfinished {
        let receipt = match locator {
            Some(locator) => crate::journal::read_process_receipt(
                &crate::journal::process_receipt_path(home, &locator),
            ),
            None => Ok(None),
        };
        let evidence = crate::journal::recorded_process_evidence(
            &process,
            &receipt,
            crate::journal::process_identity_evidence,
        );
        // No process holds the PID at all, so the recorded one has exited.
        let gone = evidence == ProcessIdentityEvidence::Dead
            || (evidence == ProcessIdentityEvidence::Unknown
                && process
                    .pid
                    .is_some_and(|pid| matches!(OsProcess::read(pid), Ok(None))));
        if !gone {
            continue;
        }
        let at = booted_at
            .filter(|booted_at| process.started_at < *booted_at)
            .unwrap_or(now);
        closed += conn.execute(
            "UPDATE processes SET completed_at=?2,
                 spawn_state=CASE WHEN kind!='agent' THEN spawn_state
                     WHEN spawn_state='reserved' AND pid IS NULL THEN 'spawn_failed' ELSE 'exited' END,
                 endpoint=NULL
             WHERE id=?1 AND completed_at IS NULL AND pid IS ?3 AND os_started_at IS ?4",
            params![process.id, at, process.pid, process.os_started_at],
        )? as u64;
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

/// Return freed pages to the filesystem. The first reclaim converts the store
/// to incremental auto-vacuum with one VACUUM; later passes free pages in place.
/// A busy store is left for the next pass.
fn reclaim(conn: &Connection) -> StoreResult<bool> {
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    let free: i64 = conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
    let mode: i64 = conn.query_row("PRAGMA auto_vacuum", [], |row| row.get(0))?;
    if mode == 2 {
        conn.execute_batch("PRAGMA incremental_vacuum;")?;
        return Ok(false);
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
    use serde_json::json;

    use super::prune;
    use crate::store::sqlite::SqliteStore;

    const INPUT: &str = "run_00000000000000000000000000000001";

    #[test]
    fn prune_keeps_facts_records_silences_and_forgets_unneeded_processes() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let session = store.test_session("conversation", INPUT);
        let line = |seq: u64, at: i64, evidence: serde_json::Value| {
            let source = format!("events.jsonl:{seq}");
            store
                .retain_session_observation(
                    &session,
                    &crate::session::SessionObservation {
                        artifact_key: INPUT.into(),
                        source: source.clone(),
                        observed_at: at,
                        task_id: None,
                        wave_id: None,
                        payload: json!({"input_id":INPUT,"source":source,"evidence":evidence}),
                    },
                )
                .unwrap();
        };
        let output =
            json!({"schema_version":1,"type":"provider_output","stream":"stdout","line":"{}"});
        let finished = json!({"schema_version":1,"type":"conversation",
            "event":{"type":"turn_completed","turn_id":"t","status":"completed"}});
        line(0, 1_000, output.clone());
        line(1, 1_100, output);
        line(2, 1_800, finished.clone());

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
        let report = prune(&path, now).unwrap();
        assert_eq!(
            (report.captures, report.stream_rows, report.processes),
            (1, 2, 3)
        );

        let events = store.input_events(INPUT).unwrap();
        assert_eq!(events, [finished]);
        assert_eq!(store.input_silences(INPUT).unwrap(), [(1_100, 1_800)]);
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

        // A second pass finds nothing left to do.
        assert_eq!(prune(&path, now).unwrap().stream_rows, 0);
    }

    #[test]
    fn prune_merges_usage_recorded_per_observation_without_changing_it() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("conversation", INPUT);
        let conn = Connection::open(&path).unwrap();
        // As earlier recorders kept it: one row per observation.
        for (seq, total, peak) in [(1, 100, 100), (2, 400, 300), (3, 500, 100)] {
            let source = format!("events.jsonl:{seq}");
            let evidence = json!({"schema_version":1,"seq":seq,"observed_at":"2026-09-29T00:00:00Z",
                "type":"usage","provider":"claude","model":null,"attempt_key":"attempt","turn_key":"turn",
                "usage_stream_id":"stream","observation_seq":seq,"counter_kind":"cumulative","start_known":true,
                "final_receipt":seq == 3,"usage":{"total_input_tokens":total,"output_tokens":total / 10,
                "peak_input_tokens":peak}});
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
                 SELECT 'conversation','observed',?1||':'||?2,1,?3,seq FROM session_events
                 WHERE kind='captured' AND receipt_key=?1",
                rusqlite::params![
                    INPUT,
                    source,
                    json!({"input_id":INPUT,"source":source,"evidence":evidence}).to_string()
                ],
            )
            .unwrap();
        }
        let counts = |input: u64| json!({"inputTokens":input,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0});
        for (total, last) in [(20, 20), (60, 40), (70, 10)] {
            let payload = json!({"total":counts(total),"last":counts(last)}).to_string();
            conn.execute(
                "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload)
                 VALUES('conversation','thread','native','usage',?1,1,?2)",
                rusqlite::params![format!("{:064}", total), payload],
            )
            .unwrap();
        }
        let before = store.input_history(INPUT).unwrap();

        let report = prune(&path, time::OffsetDateTime::now_utc().unix_timestamp()).unwrap();
        assert_eq!(report.usage_rows, 4);

        let rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM session_events WHERE kind='usage'
                 OR (kind='observed' AND json_extract(payload,'$.evidence.type')='usage')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 2);
        let native: serde_json::Value = conn
            .query_row(
                "SELECT payload FROM session_events WHERE kind='usage' AND receipt_key=''",
                [],
                |row| row.get::<_, String>(0),
            )
            .map(|payload| serde_json::from_str(&payload).unwrap())
            .unwrap();
        assert_eq!(native["total"]["inputTokens"], 70);
        assert_eq!(native["peakInputTokens"], 40);
        assert_eq!(native["baseline"]["inputTokens"], 0);
        // Only the gap a falling per-request peak counted between rows is gone.
        let after = store.input_history(INPUT).unwrap();
        assert_eq!(
            crate::session_record::SessionUsage {
                gaps: before.usage.gaps,
                ..after.usage.clone()
            },
            before.usage
        );
        assert_eq!(after.usage.peak_input_tokens, Some(300));
    }

    #[test]
    fn prune_closes_dead_processes_and_forgets_those_nothing_needs() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        SqliteStore::open_ephemeral(&path).unwrap();
        let conn = Connection::open(&path).unwrap();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        crate::journal::set_test_machine_booted_at(Some(now - 3_600));
        let exited = std::process::Command::new("true")
            .spawn()
            .and_then(|mut child| child.wait().map(|_| child.id()))
            .unwrap();
        let own = std::process::id();
        let own_start = crate::journal::process_started_at(own).unwrap();
        let id = |n: u8| format!("00000000-0000-4000-8000-0000000000{n:02}");
        let process = |n: u8, pid: Option<u32>, os: Option<i64>, at: i64, parent: Option<u8>| {
            conn.execute(
                "INSERT INTO processes(id,trace_id,pid,os_started_at,started_at,parent_lf_process_id)
                 VALUES(?1,?1,?2,?3,?4,?5)",
                rusqlite::params![id(n), pid, os, at, parent.map(id)],
            )
            .unwrap();
        };
        // Crash leftovers: once closed, only the agent-issued one stays.
        process(1, None, None, now - 7_200, None);
        process(2, Some(exited), None, now, None);
        process(3, Some(own), own_start, now, None);
        process(9, Some(exited), None, now, None);
        conn.execute(
            "UPDATE processes SET via_agent=1 WHERE id IN (?1,?2)",
            [id(1), id(2)],
        )
        .unwrap();
        let finished = |n: u8, parent: Option<u8>| {
            process(n, None, None, now, parent);
            conn.execute(
                "UPDATE processes SET completed_at=?2,outcome='succeeded' WHERE id=?1",
                rusqlite::params![id(n), now],
            )
            .unwrap();
        };
        finished(4, None);
        finished(5, None);
        finished(6, Some(3));

        let report = prune(&path, now).unwrap();
        assert_eq!((report.closed_processes, report.processes), (3, 3));
        let rows: Vec<(String, Option<i64>, Option<String>)> = conn
            .prepare("SELECT id,completed_at,outcome FROM processes ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            rows,
            [
                // A restart bounds the exit; neither exit was observed.
                (id(1), Some(now - 3_600), None),
                (id(2), Some(now), None),
                (id(3), None, None),
                (id(6), Some(now), Some("succeeded".into())),
            ]
        );
    }
}
