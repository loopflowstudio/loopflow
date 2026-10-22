//! Rows exported for another Machine and the copies this Machine keeps.
//! Export reads every table inside one savepoint, so a copy is one snapshot.
use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::{MachineId, RepositoryId, TaskId};
use crate::machine_records::{
    CopiedEvent, CopiedHistory, CopiedSession, CopyCounts, Record, RecordKind,
};
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

/// Processes started or finished in the window.
const PROCESSES: &str = "SELECT id FROM processes WHERE started_at>=?1 OR completed_at>=?1";

/// Every column the table has in this Machine's schema, by name: a copy
/// carries what its origin wrote, whatever release that was.
fn columns_in(conn: &Connection, table: &str) -> StoreResult<String> {
    let mut statement = conn.prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")?;
    let names = statement
        .query_map([table], |row| row.get::<_, String>(0))?
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
    since: i64,
) -> StoreResult<Vec<Record>> {
    let sql = format!(
        "SELECT {} FROM {table} WHERE {filter}",
        columns_in(conn, table)?
    );
    let mut statement = conn.prepare(&sql)?;
    let names: Vec<String> = statement
        .column_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut rows = statement.query([since])?;
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
        records.push(Record {
            kind,
            key,
            row: object.into(),
        });
    }
    Ok(records)
}

impl SqliteStore {
    /// This Machine's Sessions, Processes, Graph rows and session events
    /// touched since `since`, and the artifact keys of their captures.
    pub fn export_records(&self, since: i64) -> StoreResult<(Vec<Record>, Vec<String>)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let mut records = Vec::new();
        for (kind, key, table, filter) in [
            (
                RecordKind::Process,
                "id",
                "processes",
                format!("id IN ({PROCESSES})"),
            ),
            (
                RecordKind::Session,
                "id",
                "agent_sessions",
                "created_at>=?1
                    OR id IN (SELECT session_id FROM session_events WHERE observed_at>=?1)"
                    .to_string(),
            ),
            (
                RecordKind::SessionEvent,
                "seq",
                "session_events",
                "observed_at>=?1".to_string(),
            ),
            (
                RecordKind::GraphProcess,
                "lf_process_id",
                "graph_processes",
                format!("lf_process_id IN ({PROCESSES})"),
            ),
            (
                RecordKind::GraphProcessStep,
                "seq",
                "graph_process_steps",
                format!("graph_lf_process_id IN ({PROCESSES})"),
            ),
        ] {
            records.extend(rows_in(&conn, kind, key, table, &filter, since)?);
        }
        let mut statement = conn.prepare(
            "SELECT DISTINCT json_extract(payload,'$.artifact_key') FROM session_events
             WHERE kind='captured' AND observed_at>=?1
               AND json_extract(payload,'$.artifact_key') IS NOT NULL",
        )?;
        let keys = statement
            .query_map([since], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((records, keys))
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
                 DO UPDATE SET row=excluded.row, observed_at=excluded.observed_at",
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
            processes: count(RecordKind::Process)?,
        })
    }

    /// Copied Sessions, newest first, for one Task or one repository's Tasks
    /// when given. A Session's own `repo` is a path on its origin, so the
    /// repository is matched through its Task.
    pub fn copied_sessions(
        &self,
        task: Option<&TaskId>,
        repository: Option<&RepositoryId>,
    ) -> StoreResult<Vec<CopiedSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT r.origin_machine_id, m.label, r.key,
                    json_extract(r.row,'$.title'), json_extract(r.row,'$.completed_at')
             FROM machine_records r LEFT JOIN machines m ON m.id=r.origin_machine_id
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
                Ok(CopiedSession {
                    origin: MachineId::from_raw(row.get::<_, String>(0)?),
                    origin_label: row.get(1)?,
                    id: row.get(2)?,
                    title: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    completed_at: row.get(4)?,
                })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// One copied Session and its events in their origin's order, when a
    /// given-back Machine's copy holds it.
    pub fn copied_history(&self, session: &str) -> StoreResult<Option<CopiedHistory>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let found = conn
            .query_row(
                "SELECT r.origin_machine_id, m.label, r.row, r.observed_at
                 FROM machine_records r LEFT JOIN machines m ON m.id=r.origin_machine_id
                 WHERE r.kind='session' AND r.key=?1
                 ORDER BY r.observed_at DESC LIMIT 1",
                [session],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((origin, origin_label, row, copied_at)) = found else {
            return Ok(None);
        };
        let row: serde_json::Value = serde_json::from_str(&row)?;
        let mut statement = conn.prepare(
            "SELECT row FROM machine_records
             WHERE origin_machine_id=?1 AND kind='session_event'
               AND json_extract(row,'$.session_id')=?2
             ORDER BY CAST(key AS INTEGER)",
        )?;
        let events = statement
            .query_map(params![origin, session], |row| row.get::<_, String>(0))?
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
        Ok(Some(CopiedHistory {
            session: CopiedSession {
                origin: MachineId::from_raw(origin),
                origin_label,
                id: session.to_string(),
                title: text("title").unwrap_or_default(),
                completed_at: row["completed_at"].as_i64(),
            },
            task_id: text("task_id"),
            cwd: text("cwd"),
            created_at: row["created_at"].as_i64(),
            copied_at,
            events,
        }))
    }
}
