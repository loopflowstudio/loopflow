//! Retention: the store keeps facts, and streams stay in files.
//!
//! The recorder retains only the stream events history reads. Captures it
//! recorded before that rule kept every provider line; this prune brings them
//! to the same shape, records the silences their lines implied, and expires
//! inspection Processes nothing references. Each step is one short transaction
//! on its own connection, so live writers wait well under their busy timeout.

use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::session_record::SILENCE_SECONDS;
use crate::store::StoreResult;

/// Captures younger than this may still be written by an earlier lf.
const CAPTURE_SETTLE_SECONDS: i64 = 24 * 3600;
/// Finished inspection Processes stay listed this long.
pub(crate) const PROCESS_RETENTION_SECONDS: i64 = 7 * 86_400;
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
    let mut cursor = database.as_os_str().to_owned();
    cursor.push(".retention-cursor");
    let cursor = Path::new(&cursor);
    let conn = Connection::open(database)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // Nothing references the rows deleted here (see each predicate), and
    // per-row foreign-key probes would scan session_events for each delete.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let mut report = RetentionReport::default();
    prune_captures(&conn, cursor, now, &mut report)?;
    report.processes = expire_processes(&conn, now - PROCESS_RETENTION_SECONDS)?;
    report.vacuumed = reclaim(&conn)?;
    Ok(report)
}

fn prune_captures(
    conn: &Connection,
    cursor: &Path,
    now: i64,
    report: &mut RetentionReport,
) -> StoreResult<()> {
    let mut after: i64 = std::fs::read_to_string(cursor)
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0);
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
        std::fs::write(cursor, after.to_string())
            .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
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

/// Finished `lf` Processes older than `before` that nothing references: no
/// child, attachment, Session event, Graph or step, Workflow or move, Wave
/// activation, CI repair, landing supervisor, Session agent or transport,
/// relocation or steer.
fn expire_processes(conn: &Connection, before: i64) -> StoreResult<u64> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.referenced_processes;
         CREATE TEMP TABLE referenced_processes(id TEXT PRIMARY KEY) WITHOUT ROWID;
         INSERT OR IGNORE INTO temp.referenced_processes
             SELECT lf_process_id FROM session_events WHERE lf_process_id IS NOT NULL
             UNION SELECT lf_process_id FROM graph_processes WHERE lf_process_id IS NOT NULL
             UNION SELECT lf_process_id FROM graph_process_steps WHERE lf_process_id IS NOT NULL
             UNION SELECT graph_lf_process_id FROM graph_process_steps WHERE graph_lf_process_id IS NOT NULL
             UNION SELECT lf_process_id FROM task_workflows WHERE lf_process_id IS NOT NULL
             UNION SELECT lf_process_id FROM task_workflow_moves WHERE lf_process_id IS NOT NULL
             UNION SELECT project_activation_lf_process_id FROM waves WHERE project_activation_lf_process_id IS NOT NULL
             UNION SELECT repair_lf_process_id FROM ci_incidents WHERE repair_lf_process_id IS NOT NULL
             UNION SELECT supervisor_process_id FROM pr_landings WHERE supervisor_process_id IS NOT NULL
             UNION SELECT agent_process_id FROM agent_sessions WHERE agent_process_id IS NOT NULL
             UNION SELECT transport_lf_process_id FROM agent_sessions WHERE transport_lf_process_id IS NOT NULL
             UNION SELECT process_id FROM relocated_task_processes WHERE process_id IS NOT NULL
             UNION SELECT agent_process_id FROM steer_adapters WHERE agent_process_id IS NOT NULL
             UNION SELECT agent_process_id FROM steer_batches WHERE agent_process_id IS NOT NULL;",
    )?;
    let mut expired = 0;
    loop {
        let deleted = conn.execute(
            "DELETE FROM processes WHERE id IN (
                 SELECT p.id FROM processes p
                 WHERE p.started_at<?1 AND p.kind='lf' AND p.completed_at IS NOT NULL AND p.completed_at<?1
                   AND p.id NOT IN (SELECT id FROM temp.referenced_processes)
                   AND NOT EXISTS(SELECT 1 FROM processes c WHERE c.parent_lf_process_id=p.id)
                   AND NOT EXISTS(SELECT 1 FROM processes a WHERE a.kind='agent' AND a.attached_lf_process_id=p.id)
                 LIMIT ?2)",
            params![before, PROCESS_BATCH],
        )?;
        expired += deleted as u64;
        if (deleted as i64) < PROCESS_BATCH {
            break;
        }
    }
    conn.execute_batch("DROP TABLE temp.referenced_processes;")?;
    Ok(expired)
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

    use super::{prune, PROCESS_RETENTION_SECONDS};
    use crate::store::sqlite::SqliteStore;

    const INPUT: &str = "run_00000000000000000000000000000001";

    #[test]
    fn prune_keeps_facts_records_silences_and_expires_unreferenced_processes() {
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
        let old = now - PROCESS_RETENTION_SECONDS - 60;
        let conn = Connection::open(&path).unwrap();
        let process = |id: &str, parent: Option<&str>, at: i64| {
            conn.execute(
                "INSERT INTO processes(id,trace_id,parent_lf_process_id,started_at,completed_at,outcome)
                 VALUES(?1,?1,?2,?3,?3,'succeeded')",
                rusqlite::params![id, parent, at],
            )
            .unwrap();
        };
        let id = |n: u8| format!("00000000-0000-4000-8000-0000000000{n:02}");
        process(&id(1), None, old);
        process(&id(2), None, old);
        process(&id(3), Some(&id(2)), now);
        process(&id(4), None, now - 60);

        let report = prune(&path, now).unwrap();
        assert_eq!(
            (report.captures, report.stream_rows, report.processes),
            (1, 2, 1)
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
        assert_eq!(left, [id(2), id(3), id(4)]);
        let violations: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);

        // A second pass finds nothing left to do.
        assert_eq!(prune(&path, now).unwrap().stream_rows, 0);
    }
}
