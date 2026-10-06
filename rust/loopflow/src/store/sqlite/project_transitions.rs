use rusqlite::{params, OptionalExtension};

use crate::id::WaveId;
use crate::store::project_transitions::ProjectTransition;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

impl SqliteStore {
    pub(crate) fn pending_project_transition(
        &self,
        wave: &WaveId,
    ) -> StoreResult<Option<ProjectTransition>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT successor_id, predecessor_id, reset_name, created_at, settled_at
             FROM project_transitions WHERE wave_id=?1 AND settled_at IS NULL",
                [wave.as_str()],
                |row| {
                    Ok(ProjectTransition {
                        wave_id: wave.clone(),
                        successor_id: row.get(0)?,
                        predecessor_id: row.get(1)?,
                        reset_name: row.get(2)?,
                        created_at: row.get(3)?,
                        settled_at: row.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    pub(crate) fn reserve_project_transition(
        &self,
        transition: &ProjectTransition,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO project_transitions
             (wave_id, successor_id, predecessor_id, reset_name, created_at, settled_at)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
            params![
                transition.wave_id.as_str(),
                transition.successor_id,
                transition.predecessor_id,
                transition.reset_name,
                transition.created_at
            ],
        )?;
        Ok(())
    }

    pub(crate) fn settle_project_transition(
        &self,
        wave: &WaveId,
        successor: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE project_transitions SET settled_at=COALESCE(settled_at, ?3)
             WHERE wave_id=?1 AND successor_id=?2",
            params![
                wave.as_str(),
                successor,
                time::OffsetDateTime::now_utc().unix_timestamp()
            ],
        )?;
        if updated != 1 {
            return Err(StoreError::InvalidData(
                "Project transition is missing".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn project_ensure_migration_keeps_one_pending_transition_and_settled_history() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::migrations::apply_before_current_draft(&conn, "project_readiness");
        conn.execute_batch(
            "INSERT INTO waves(id,name,repo,created_at) VALUES('w','product','/repo',1)",
        )
        .unwrap();
        conn.execute_batch(&crate::store::migrations::current_draft_sql(
            "project_readiness",
        ))
        .unwrap();
        conn.execute("INSERT INTO project_transitions(wave_id,successor_id,created_at) VALUES('w','first',1)", []).unwrap();
        assert!(conn.execute("INSERT INTO project_transitions(wave_id,successor_id,created_at) VALUES('w','second',2)", []).is_err());
        conn.execute(
            "UPDATE project_transitions SET settled_at=3 WHERE wave_id='w'",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO project_transitions(wave_id,predecessor_id,successor_id,reset_name,created_at) VALUES('w','first','second','Autumn',4)", []).unwrap();
        let evidence: (i64, i64) = conn
            .query_row(
                "SELECT count(*),sum(settled_at IS NULL) FROM project_transitions",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(evidence, (2, 1));
        assert_eq!(
            conn.query_row(
                "SELECT predecessor_id FROM project_transitions WHERE settled_at IS NULL",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "first"
        );
    }
}
