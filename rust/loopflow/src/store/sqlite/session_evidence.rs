//! History owns the raw-reference projection. Coverage is transactional; paths
//! remain unresolved here so every cleanup admission sees current symlinks.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, TransactionBehavior};

use super::SqliteStore;
use crate::store::{StoreError, StoreResult};

const PAGE_SIZE: usize = 256;

fn bounded<T>(conn: &Connection, read: impl FnOnce() -> StoreResult<T>) -> StoreResult<T> {
    let busy: u32 = conn.pragma_query_value(None, "busy_timeout", |row| row.get(0))?;
    conn.busy_timeout(Duration::ZERO)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    if let Err(error) = conn.progress_handler(1000, Some(move || Instant::now() >= deadline)) {
        conn.busy_timeout(Duration::from_millis(u64::from(busy)))?;
        return Err(error.into());
    }
    let result = read();
    let cleared = conn.progress_handler(0, None::<fn() -> bool>);
    conn.busy_timeout(Duration::from_millis(u64::from(busy)))?;
    cleared?;
    result
}

impl SqliteStore {
    /// One maintenance page, committed with its coverage cursor. Interrupted
    /// pages roll back; new evidence is projected by the source's own triggers.
    /// Previews never invoke this writer.
    pub(crate) fn advance_session_evidence(&self) -> StoreResult<()> {
        let conn = self
            .conn
            .try_lock()
            .map_err(|_| StoreError::InvalidData("Session evidence reader is busy".into()))?;
        // Use an unchecked transaction inside the bounded connection borrow;
        // this function owns its mutex and never nests transactions.
        bounded(&conn, || {
            let tx = rusqlite::Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)?;
            let (through, target, complete): (i64, i64, bool) = tx.query_row(
                "SELECT through_seq,target_seq,complete FROM session_evidence_backfill WHERE singleton=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            if complete {
                return Ok(());
            }
            let rows = {
                let mut query = tx.prepare(
                    "SELECT seq,capture_key,raw_paths FROM session_evidence_source WHERE seq>?1 AND seq<=?2 ORDER BY seq LIMIT ?3",
                )?;
                let rows = query
                    .query_map(params![through, target, PAGE_SIZE as i64], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            };
            for (seq, key, paths) in &rows {
                tx.execute(
                    "INSERT OR REPLACE INTO session_evidence(event_seq,capture_key,raw_paths) VALUES(?1,?2,?3)",
                    params![seq, key, paths],
                )?;
            }
            tx.execute(
                "UPDATE session_evidence_backfill SET through_seq=?1,complete=?2 WHERE singleton=1",
                params![
                    rows.last().map_or(through, |row| row.0),
                    rows.len() < PAGE_SIZE
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    /// Stream raw references newer than `after` from one snapshot, advancing it
    /// past every consumed row. None marks a newly consumed row; Some is one
    /// reference to resolve freshly. A positive visitor stops immediately and
    /// only complete coverage may return false. Edits re-project a reference at
    /// a later revision, so a caller repeats until a snapshot adds nothing.
    pub(crate) fn visit_session_evidence(
        &self,
        after: &mut i64,
        mut visit: impl FnMut(Option<&Path>) -> StoreResult<bool>,
    ) -> StoreResult<bool> {
        let conn = self
            .conn
            .try_lock()
            .map_err(|_| StoreError::InvalidData("Session evidence reader is busy".into()))?;
        let tx = conn.unchecked_transaction()?;
        let complete: bool = tx.query_row(
            "SELECT complete FROM session_evidence_backfill WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        if !complete {
            return Err(StoreError::InvalidData(
                "Session evidence backfill incomplete".into(),
            ));
        }
        let home = super::home_dir_in(&tx)?;
        let mut query = tx.prepare(
            "SELECT revision,capture_key,raw_paths FROM session_evidence WHERE revision>?1 ORDER BY revision",
        )?;
        let mut rows = query.query([*after])?;
        while let Some(row) = rows.next()? {
            if visit(None)? {
                return Ok(true);
            }
            let key: Option<String> = row.get(1)?;
            let dir = key
                .as_deref()
                .and_then(|key| crate::session_record::record_dir(&home, key))
                .ok_or_else(|| {
                    StoreError::InvalidData("Session evidence has no capture directory".into())
                })?;
            if visit(Some(&dir))? {
                return Ok(true);
            }
            // These are published files, not just current Session pointers.
            for name in [
                "manifest.json",
                "context.json",
                "events.jsonl",
                "provider.jsonl",
                "provider-session.json",
                "conversation.jsonl",
                "terminal.json",
            ] {
                if visit(Some(&dir.join(name)))? {
                    return Ok(true);
                }
            }
            let references: Vec<Option<String>> = serde_json::from_str(&row.get::<_, String>(2)?)?;
            for value in references
                .into_iter()
                .flatten()
                .filter(|value| !value.is_empty())
            {
                let path = PathBuf::from(value);
                let path = if path.is_absolute() {
                    path
                } else {
                    dir.join(path)
                };
                if visit(Some(&path))? {
                    return Ok(true);
                }
            }
            *after = row.get(0)?;
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use std::path::{Path, PathBuf};

    use rusqlite::{params, Connection};

    use super::{SqliteStore, PAGE_SIZE};

    fn paths(store: &SqliteStore, after: &mut i64) -> crate::store::StoreResult<Vec<PathBuf>> {
        let mut paths = Vec::new();
        store.visit_session_evidence(after, |path| {
            paths.extend(path.map(Path::to_path_buf));
            Ok(false)
        })?;
        Ok(paths)
    }

    #[test]
    fn cleanup_observation_deadline_leaves_normal_history_writes_available() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let reader = store
            .bounded_reader(std::time::Duration::from_millis(1))
            .unwrap();
        let result = reader.conn.lock().unwrap().query_row(
            "WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM n WHERE i<100000000) SELECT SUM(i) FROM n", [], |row| row.get::<_, i64>(0),
        );
        assert!(result.is_err());
        let session = store.test_session("after-timeout", "00000000000000000000000000000001");
        assert_eq!(session.id, "after-timeout");
        assert!(!paths(&store, &mut 0).unwrap().is_empty());
        let _busy = store.conn.lock().unwrap();
        assert!(store
            .bounded_reader(std::time::Duration::from_secs(2))
            .is_err());
    }

    #[test]
    fn cleanup_history_projection_upgrades_in_pages_and_tracks_changes_to_earlier_pages() {
        let home = tempfile::tempdir().unwrap();
        let conn = Connection::open(home.path().join("store.db")).unwrap();
        crate::store::migrations::apply_before_current_draft(&conn, "session_evidence_projection");
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES('past','past','generated',1,1,'/')", []).unwrap();
        for index in 0..(PAGE_SIZE * 2 + 1) {
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','captured',?1,1,'{}')", [format!("{index:032x}")]).unwrap();
        }
        let input = format!("{:032x}", 0);
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','observed',?1,1,?2)",
            params![format!("{input}:runs"), r#"{"evidence":{"provider_session_path":"/before"}}"#]).unwrap();
        conn.execute_batch(&crate::store::migrations::current_draft_sql(
            "session_evidence_projection",
        ))
        .unwrap();
        let store = SqliteStore {
            conn: Arc::new(Mutex::new(conn)),
        };
        assert!(paths(&store, &mut 0).is_err());
        store.advance_session_evidence().unwrap();
        assert!(paths(&store, &mut 0).is_err());
        {
            let conn = store.conn.lock().unwrap();
            // Arrivals are already projected atomically; they must not extend
            // the historical backfill cohort, even above its per-tick budget.
            for index in 10000..(10000 + PAGE_SIZE * 2) {
                conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','captured',?1,1,'{}')", [format!("{index:032x}")]).unwrap();
            }
        }
        // A failed transaction must not skip the uncommitted portion of a page.
        {
            let conn = store.conn.lock().unwrap();
            conn.execute_batch("CREATE TEMP TRIGGER interrupt_projection BEFORE INSERT ON session_evidence WHEN NEW.event_seq>300 BEGIN SELECT RAISE(ABORT,'interrupted page'); END;").unwrap();
        }
        assert!(store.advance_session_evidence().is_err());
        {
            let conn = store.conn.lock().unwrap();
            assert_eq!(
                conn.query_row(
                    "SELECT through_seq FROM session_evidence_backfill",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                PAGE_SIZE as i64
            );
            conn.execute_batch("DROP TRIGGER interrupt_projection")
                .unwrap();
        }
        store.advance_session_evidence().unwrap();
        assert!(paths(&store, &mut 0).is_err());
        store.advance_session_evidence().unwrap();
        let mut consumed = 0;
        assert!(paths(&store, &mut consumed)
            .unwrap()
            .contains(&"/before".into()));
        {
            let conn = store.conn.lock().unwrap();
            // Old references can change; source triggers invalidate them atomically.
            conn.execute(
                "UPDATE session_events SET payload=?1 WHERE receipt_key=?2",
                params![
                    r#"{"evidence":{"provider_session_path":"/after"}}"#,
                    format!("{input}:runs")
                ],
            )
            .unwrap();
            // Appending an observation of an old capture cannot hide behind its cursor.
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','observed',?1,1,?2)",
                params![format!("{input}:terminal.json"), r#"{"evidence":{"result_ref":"/appended"}}"#]).unwrap();
        }
        // An observation that already passed these rows revalidates exactly
        // what changed behind it, without restarting or trusting its snapshot.
        let changed = paths(&store, &mut consumed).unwrap();
        assert!(changed.contains(&"/after".into()) && changed.contains(&"/appended".into()));
        assert!(!changed.contains(&"/before".into()));
        assert!(paths(&store, &mut consumed).unwrap().is_empty());
        let current = paths(&store, &mut 0).unwrap();
        assert!(!current.contains(&"/before".into()));
        assert!(current.contains(&"/after".into()));
        assert!(current.contains(&"/appended".into()));
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE session_events SET payload=?1 WHERE receipt_key=?2",
                params![
                    r#"{"evidence":{"provider_session_path":17}}"#,
                    format!("{input}:runs")
                ],
            )
            .unwrap();
        }
        assert!(paths(&store, &mut 0).is_err());
    }
}
