//! Rows exported for another Machine and the copies this Machine keeps.
//! Both databases are read in a pinned snapshot for each Session.
use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::de::DeserializeOwned;

use crate::machine_records::{
    CopiedEvent, CopiedHistory, CopiedSession, CopyCounts, CopyCursor, Record, RecordKind,
};
use crate::rows::now_unix;
use crate::{StoreError, StoreResult};
use loopflow_model::durable::{MachineId, RepositoryId, TaskId};

use super::SqliteStore;

/// Dispatch attribution survives transcript replay on a different Machine.
/// For otherwise equivalent readings, prefer local, then stable Machine ID order.
pub(super) const COLLECTED_TURNS: &str = "
    SELECT c.* FROM machine_record_inputs c
    LEFT JOIN history.turns t ON t.session_id=c.session_id
        AND t.provider_thread=json_extract(c.row,'$.thread') AND t.provider_turn=json_extract(c.row,'$.turn')
    WHERE c.kind='turn' AND (t.id IS NULL OR (t.lf_process_id IS NULL AND t.captured_event IS NULL
        AND json_extract(c.row,'$.lf_process_id') IS NOT NULL))
      AND c.origin_machine_id=(
        SELECT other.origin_machine_id FROM machine_records other WHERE other.kind='turn' AND other.key=c.key
        ORDER BY (json_extract(other.row,'$.lf_process_id') IS NOT NULL) DESC,other.origin_machine_id LIMIT 1)";

/// A copy carries the columns its origin actually has, including generated history kinds.
fn columns_in(conn: &Connection, table: &str) -> StoreResult<String> {
    let (schema, table) = table.split_once('.').unwrap_or(("main", table));
    let mut statement =
        conn.prepare_cached("SELECT name FROM pragma_table_xinfo(?1,?2) ORDER BY cid")?;
    let names = statement
        .query_map([table, schema], |row| row.get::<_, String>(0))?
        .map(|name| name.map(|name| format!("\"{}\"", name.replace('"', "\"\""))))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names.join(","))
}

fn rows_in(
    conn: &Connection,
    kind: RecordKind,
    key: &str,
    table: &str,
    filter: &str,
    session: &str,
) -> StoreResult<Vec<Record>> {
    let columns = columns_in(conn, table)?;
    let sql = format!("SELECT {columns} FROM {table} WHERE {filter}");
    let mut statement = conn.prepare(&sql)?;
    let names: Vec<String> = statement
        .column_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut rows = statement.query([session])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        let mut object = serde_json::Map::new();
        for (index, name) in names.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => serde_json::Value::Null,
                ValueRef::Integer(value) => value.into(),
                ValueRef::Real(value) => value.into(),
                ValueRef::Text(text) => String::from_utf8_lossy(text).into_owned().into(),
                ValueRef::Blob(_) => {
                    return Err(StoreError::InvalidData(format!(
                        "{} column {name} holds a blob, which exports cannot carry",
                        kind.as_str()
                    )))
                }
            };
            object.insert(name.clone(), value);
        }
        let key = match object.get(key) {
            Some(serde_json::Value::String(text)) => text.clone(),
            Some(serde_json::Value::Number(number)) => number.to_string(),
            _ => {
                return Err(StoreError::InvalidData(format!(
                    "{} row has no {key}",
                    kind.as_str()
                )))
            }
        };
        let key = if kind == RecordKind::History {
            serde_json::to_string(&(session, &object["receipt_key"]))?
        } else {
            key
        };
        records.push(Record {
            kind,
            key,
            row: object.into(),
        });
    }
    Ok(records)
}

fn copied_rows<T: DeserializeOwned>(
    conn: &Connection,
    origin: &MachineId,
    kind: RecordKind,
    session: &str,
) -> StoreResult<Vec<T>> {
    let mut stmt = conn.prepare(
        "SELECT row FROM machine_records WHERE origin_machine_id=?1 AND kind=?2
        AND json_extract(row,'$.session_id')=?3",
    )?;
    let rows = stmt.query_map(params![origin.as_str(), kind.as_str(), session], |row| {
        row.get::<_, String>(0)
    })?;
    rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
}

impl SqliteStore {
    /// Upgrade an older history file before taking the export snapshot.
    /// Normal reads tolerate a missing history file. A final copy must see the real one.
    pub fn prepare_history_export(&self) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let file: String = conn.query_row(
            "SELECT file FROM pragma_database_list WHERE name='history'",
            [],
            |row| row.get(0),
        )?;
        if file.is_empty() {
            return Err(StoreError::InvalidData(
                "history.db is unavailable for export".into(),
            ));
        }
        let current: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM history.sqlite_schema WHERE name='session_export_changes')",
            [], |row| row.get(0),
        )?;
        if !current {
            let store = conn.path().expect("the exporter uses a file-backed store");
            crate::history::open_writer(std::path::Path::new(store))?;
        }
        Ok(())
    }

    pub fn export_cursor(&self) -> StoreResult<CopyCursor> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT
            COALESCE((SELECT MAX(position) FROM session_export_changes),0),
            COALESCE((SELECT MAX(position) FROM history.session_export_changes),0)",
            [],
            |row| {
                Ok(CopyCursor {
                    state: row.get(0)?,
                    history: row.get(1)?,
                    started_at: now_unix(),
                })
            },
        )?)
    }

    /// The first copy includes Sessions touched in the requested window.
    /// Thereafter positions track writes, even to a turn dated before that window.
    pub fn export_sessions(
        &self,
        since: i64,
        after: Option<CopyCursor>,
    ) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (sql, bounds) = match after {
            Some(after) => ("SELECT session_id FROM session_export_changes WHERE position>?1
                UNION SELECT session_id FROM history.session_export_changes WHERE position>?2",
                vec![after.state, after.history]),
            None => ("SELECT id FROM agent_sessions WHERE created_at>=?1 OR completed_at>=?1
                UNION SELECT session_id FROM session_events WHERE observed_at>=?1
                UNION SELECT session_id FROM history.session_history WHERE observed_at>=?1
                UNION SELECT session_id FROM history.turns WHERE started_at>=?1 OR ended_at>=?1 OR usage_at>=?1",
                vec![since]),
        };
        let mut statement = conn.prepare(sql)?;
        let sessions = statement
            .query_map(rusqlite::params_from_iter(bounds), |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(sessions)
    }

    pub fn open_transcripts(&self) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare("SELECT id FROM agent_sessions WHERE completed_at IS NULL AND provider_thread IS NOT NULL")?;
        let ids = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    pub fn export_session_records(&self, session: &str) -> StoreResult<Vec<Record>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut records = Vec::new();
        for (kind, key, table, filter) in [
            (RecordKind::Session, "id", "agent_sessions", "id=?1"),
            (
                RecordKind::SessionEvent,
                "seq",
                "session_events",
                "session_id=?1",
            ),
            (
                RecordKind::History,
                "seq",
                "history.session_history",
                "session_id=?1",
            ),
        ] {
            records.extend(rows_in(&conn, kind, key, table, filter, session)?);
        }
        Ok(records)
    }

    pub fn copied_cursor(&self, origin: &MachineId) -> StoreResult<Option<CopyCursor>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let row: Option<String> = conn
            .query_row(
                "SELECT row FROM machine_records
            WHERE origin_machine_id=?1 AND kind='checkpoint' AND key='export'",
                [origin.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        row.map(|row| serde_json::from_str(&row).map_err(Into::into))
            .transpose()
    }

    pub fn copied_session_rows<T: DeserializeOwned>(
        &self,
        origin: &MachineId,
        kind: RecordKind,
        session: &str,
    ) -> StoreResult<Vec<T>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        copied_rows(&conn, origin, kind, session)
    }

    /// Visit transcript rows once per origin Session with an input in the requested Work and window.
    /// The visitor can stop the SQLite cursor as soon as the caller has enough hits.
    pub fn visit_copied_transcripts(
        &self,
        since: i64,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
        mut visit: impl FnMut(MachineId, &[u8]) -> StoreResult<std::ops::ControlFlow<()>>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let task_id = task
            .map(|selector| super::children::resolve_task_id_in(&snapshot, selector, None, None))
            .transpose()?
            .flatten();
        let task = task_id.as_ref().map(TaskId::as_str).or(task);
        let task_filter = if task.is_some() {
            "AND task_id=?7"
        } else {
            "AND ?7 IS NULL"
        };
        let sql = format!(
            "WITH sessions AS (
                SELECT DISTINCT origin_machine_id, session_id AS session FROM machine_record_inputs
                WHERE started>=?1 {task_filter}
                  AND (?2 IS NULL OR scope_wave IN
                      (SELECT id FROM wave_addresses WHERE slug=?2 OR id=?3))
                  AND (?4 IS NULL OR project_slug=?4 OR project_id=?5 OR external_project_id=?6)
            )
            SELECT s.origin_machine_id, tr.row FROM sessions s CROSS JOIN machine_records tr
            WHERE tr.origin_machine_id=s.origin_machine_id AND tr.kind='transcript'
              AND json_extract(tr.row,'$.session_id')=s.session"
        );
        let mut statement = snapshot.prepare(&sql)?;
        let mut rows = statement.query(params![
            since,
            wave,
            wave.map(loopflow_model::id::canonical_uuid),
            project,
            project.map(|p| loopflow_model::id::canonical_prefixed(p, "proj_")),
            project.map(loopflow_model::id::canonical_uuid),
            task,
        ])?;
        while let Some(row) = rows.next()? {
            let origin = MachineId::from_raw(row.get::<_, String>(0)?);
            let body = row.get_ref(1)?.as_bytes().map_err(rusqlite::Error::from)?;
            if visit(origin, body)?.is_break() {
                break;
            }
        }
        Ok(())
    }

    /// Keep `origin`'s rows as copies, replacing older copies of the same rows.
    pub fn import_records(&self, origin: &MachineId, records: &[Record]) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if &super::durable::map_local_machine(&tx)?.id == origin {
            return Err(StoreError::InvalidData(
                "this Machine's own records are not copies".into(),
            ));
        }
        let now = now_unix();
        for record in records {
            tx.execute(
                "INSERT INTO machine_records(origin_machine_id,kind,key,row,observed_at)
                 VALUES(?1,?2,?3,?4,?5)
                 ON CONFLICT(origin_machine_id,kind,key)
                 DO UPDATE SET row=excluded.row, observed_at=excluded.observed_at
                 WHERE machine_records.row<>excluded.row OR excluded.kind='checkpoint'",
                params![
                    origin.as_str(),
                    record.kind.as_str(),
                    record.key,
                    serde_json::to_string(&record.row)?,
                    now
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn copy_counts(&self, origin: &MachineId) -> StoreResult<CopyCounts> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let count = |kind: RecordKind| -> StoreResult<usize> {
            Ok(conn.query_row(
                "SELECT count(*) FROM machine_records WHERE origin_machine_id=?1 AND kind=?2",
                params![origin.as_str(), kind.as_str()],
                |row| row.get::<_, i64>(0),
            )? as usize)
        };
        Ok(CopyCounts {
            sessions: count(RecordKind::Session)?,
        })
    }

    /// Copied Sessions, newest first, for one Task or one repository's Tasks
    /// when given. A Session's own `repo` is a path on its origin, so the
    /// repository is matched through its Task.
    pub fn saved_copied_sessions(
        &self,
        task: Option<&TaskId>,
        repository: Option<&RepositoryId>,
    ) -> StoreResult<Vec<CopiedSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT r.origin_machine_id, r.key,
                    json_extract(r.row,'$.title'), json_extract(r.row,'$.completed_at')
             FROM machine_records r
             WHERE r.kind='session'
               AND (?1 IS NULL OR json_extract(r.row,'$.task_id')=?1)
               AND (?2 IS NULL OR json_extract(r.row,'$.task_id')
                    IN (SELECT id FROM tasks WHERE repository_id=?2))
             ORDER BY json_extract(r.row,'$.created_at') DESC, r.key",
        )?;
        let rows = statement.query_map(
            params![
                task.map(TaskId::as_str),
                repository.map(RepositoryId::as_str)
            ],
            |row| {
                let origin = MachineId::from_raw(row.get::<_, String>(0)?);
                Ok(CopiedSession {
                    origin,
                    origin_label: None,
                    id: row.get(1)?,
                    title: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    completed_at: row.get(3)?,
                })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Read the additional native turns on one origin, in a Session drill-down.
    pub fn collected_turns(
        &self,
        origin: &MachineId,
        session: &str,
    ) -> StoreResult<Vec<crate::session::Turn>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(&format!(
            "SELECT row FROM ({COLLECTED_TURNS}) WHERE origin_machine_id=?1 AND session_id=?2"
        ))?;
        let rows = statement.query_map(params![origin.as_str(), session], |row| {
            row.get::<_, String>(0)
        })?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }

    /// Machines holding a copy of the Session, and their last completed collection.
    pub fn copied_session_origins(&self, session: &str) -> StoreResult<Vec<(MachineId, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT s.origin_machine_id,coalesce(c.observed_at,s.observed_at) FROM machine_records s
             LEFT JOIN machine_records c ON c.origin_machine_id=s.origin_machine_id
                AND c.kind='checkpoint' AND c.key='export'
             WHERE s.kind='session' AND s.key=?1 ORDER BY s.origin_machine_id",
        )?;
        let rows = stmt.query_map([session], |row| {
            Ok((MachineId::from_raw(row.get::<_, String>(0)?), row.get(1)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// One copied Session and its events in their origin's order, when a
    /// given-back Machine's copy holds it.
    pub fn saved_copied_history(&self, session: &str) -> StoreResult<Option<CopiedHistory>> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let found = conn
            .query_row(
                "SELECT origin_machine_id, row, observed_at
                 FROM machine_records
                 WHERE kind='session' AND key=?1
                 ORDER BY observed_at DESC LIMIT 1",
                [session],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((origin, row, copied_at)) = found else {
            return Ok(None);
        };
        let origin = MachineId::from_raw(origin);
        let row: serde_json::Value = serde_json::from_str(&row)?;
        let mut statement = conn.prepare(
            "SELECT row FROM machine_records
             WHERE origin_machine_id=?1 AND kind IN ('session_event','history')
               AND json_extract(row,'$.session_id')=?2
             ORDER BY json_extract(row,'$.observed_at'), kind, json_extract(row,'$.seq')",
        )?;
        let events = statement
            .query_map(params![origin.as_str(), session], |row| {
                row.get::<_, String>(0)
            })?
            .map(|row| -> StoreResult<CopiedEvent> {
                let row: serde_json::Value = serde_json::from_str(&row?)?;
                let payload = row["payload"]
                    .as_str()
                    .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
                Ok(CopiedEvent {
                    kind: row["kind"].as_str().unwrap_or_default().to_string(),
                    observed_at: row["observed_at"].as_i64(),
                    artifact_key: payload
                        .as_ref()
                        .and_then(|payload| payload["artifact_key"].as_str())
                        .map(str::to_string),
                    row,
                })
            })
            .collect::<StoreResult<Vec<_>>>()?;
        let text = |name: &str| row[name].as_str().map(str::to_string);
        let mut turns: Vec<crate::session::Turn> =
            copied_rows(&conn, &origin, RecordKind::Turn, session)?;
        turns.sort_by_key(|turn| turn.id);
        let last_collection: Option<i64> = conn
            .query_row(
                "SELECT observed_at FROM machine_records
             WHERE origin_machine_id=?1 AND kind='checkpoint' AND key='export'",
                [origin.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(Some(CopiedHistory {
            usage: crate::session_record::reduce_usage(&turns),
            turns,
            session: CopiedSession {
                origin,
                origin_label: None,
                id: session.to_string(),
                title: text("title").unwrap_or_default(),
                completed_at: row["completed_at"].as_i64(),
            },
            task_id: text("task_id"),
            cwd: text("cwd"),
            created_at: row["created_at"].as_i64(),
            copied_at: last_collection.unwrap_or(copied_at),
            events,
        }))
    }
}
