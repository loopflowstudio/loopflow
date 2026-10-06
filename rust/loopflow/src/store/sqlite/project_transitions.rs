use rusqlite::{params, OptionalExtension};

use crate::id::WaveId;
use crate::store::project_transitions::ProjectTransition;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

impl SqliteStore {
    pub(crate) fn project_transition(
        &self,
        wave: &WaveId,
        successor: &str,
    ) -> StoreResult<Option<ProjectTransition>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT predecessor_id, reset_name, created_at, settled_at, create_successor FROM project_transitions
             WHERE wave_id=?1 AND successor_id=?2", params![wave.as_str(), successor],
            |row| Ok(ProjectTransition {
                wave_id: wave.clone(), successor_id: successor.to_owned(),
                predecessor_id: row.get(0)?, reset_name: row.get(1)?,
                created_at: row.get(2)?, settled_at: row.get(3)?, create_successor: row.get(4)?,
            }),
        ).optional()?)
    }

    pub(crate) fn project_transition_items(
        &self,
        wave: &WaveId,
        successor: &str,
    ) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT issue_id FROM project_transition_items
            WHERE wave_id=?1 AND successor_id=?2 ORDER BY issue_id",
        )?;
        let items = query
            .query_map(params![wave.as_str(), successor], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(items)
    }

    pub(crate) fn select_project_transition_item(
        &self,
        wave: &WaveId,
        successor: &str,
        issue: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let pending: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_transitions
            WHERE wave_id=?1 AND successor_id=?2 AND settled_at IS NULL)",
            params![wave.as_str(), successor],
            |row| row.get(0),
        )?;
        if !pending {
            return Err(StoreError::InvalidData(
                "Project transition is not pending".into(),
            ));
        }
        conn.execute(
            "INSERT OR IGNORE INTO project_transition_items(wave_id,successor_id,issue_id)
            VALUES(?1,?2,?3)",
            params![wave.as_str(), successor, issue],
        )?;
        Ok(())
    }

    pub(crate) fn pending_project_transition(
        &self,
        wave: &WaveId,
    ) -> StoreResult<Option<ProjectTransition>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT successor_id, predecessor_id, reset_name, created_at, settled_at, create_successor
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
                        create_successor: row.get(5)?,
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
             (wave_id, successor_id, predecessor_id, reset_name, created_at, settled_at, create_successor)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
            params![
                transition.wave_id.as_str(),
                transition.successor_id,
                transition.predecessor_id,
                transition.reset_name,
                transition.created_at,
                transition.create_successor
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
             WHERE wave_id=?1 AND successor_id=?2
               AND EXISTS(SELECT 1 FROM waves w JOIN projects p ON p.id=w.current_project_id
                          WHERE w.id=?1 AND p.external_project_id=?2)",
            params![
                wave.as_str(),
                successor,
                time::OffsetDateTime::now_utc().unix_timestamp()
            ],
        )?;
        if updated != 1 {
            return Err(StoreError::InvalidData(
                "Project binding changed before settlement or transition is missing; preserve the intervening decision".into(),
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
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        crate::store::migrations::apply_before_current_draft(&conn, "project_readiness");
        conn.execute_batch(
            "INSERT INTO waves(id,name,repo,created_at) VALUES('w','product','/repo',1)",
        )
        .unwrap();
        conn.execute_batch(&crate::store::migrations::current_draft_sql(
            "project_readiness",
        ))
        .unwrap();
        // The finished schema includes the reactive-stream dependency too.
        if !conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='store_revisions')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap()
        {
            conn.execute_batch(&crate::store::migrations::current_draft_sql(
                "store_revisions",
            ))
            .unwrap();
        }
        conn.execute("INSERT INTO project_transitions(wave_id,successor_id,created_at) VALUES('w','first',1)", []).unwrap();
        assert!(conn.execute("INSERT INTO project_transitions(wave_id,successor_id,created_at) VALUES('w','second',2)", []).is_err());
        conn.execute(
            "UPDATE project_transitions SET settled_at=3 WHERE wave_id='w'",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO project_transitions(wave_id,predecessor_id,successor_id,reset_name,created_at,create_successor) VALUES('w','first','second','Autumn',4,0)", []).unwrap();
        conn.execute("INSERT INTO project_transition_items(wave_id,successor_id,issue_id) VALUES('w','second','selected')", []).unwrap();
        assert!(conn.execute("INSERT INTO project_transition_items(wave_id,successor_id,issue_id) VALUES('w','missing','selected')", []).is_err());
        conn.execute(
            "UPDATE project_transitions SET settled_at=5 WHERE successor_id='second'",
            [],
        )
        .unwrap();
        assert_eq!(conn.query_row("SELECT issue_id FROM project_transition_items WHERE wave_id='w' AND successor_id='second'", [], |row| row.get::<_, String>(0)).unwrap(), "selected");
        let evidence: (i64, i64) = conn
            .query_row(
                "SELECT count(*),sum(settled_at IS NULL) FROM project_transitions",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(evidence, (2, 0));
        assert_eq!(
            conn.query_row(
                "SELECT predecessor_id FROM project_transitions WHERE successor_id='second'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "first"
        );
    }
}
