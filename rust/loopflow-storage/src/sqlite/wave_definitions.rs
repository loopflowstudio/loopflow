//! Saved Wave bootstrap inputs and atomic hierarchy registration.

use super::repositories::RepositoryRegistration;
use super::{planning_write, SqliteStore};
use crate::work::edit::WorkValue as Edit;
use crate::work::wave::{Wave, WaveLocator};
use crate::StoreResult;
use loopflow_model::durable::RepositoryId;
use loopflow_model::id::WaveId;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveBootstrapRow {
    pub id: WaveId,
    pub repo: String,
    pub name: String,
    pub parent: Option<WaveId>,
    pub retired_at: Option<i64>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct WaveBootstrapInputs {
    pub repository: RepositoryRegistration,
    pub waves: Vec<WaveBootstrapRow>,
}

const WAVE_BOOTSTRAP_SELECT: &str =
    "SELECT id,repo,name,parent_wave_id,retired_at FROM waves WHERE repo=?1 AND retired_at IS NULL
     UNION SELECT id,repo,name,parent_wave_id,retired_at FROM waves
     WHERE id IN (SELECT value FROM json_each(?2)) ORDER BY id";

fn bootstrap_inputs_in(
    conn: &Connection,
    repo: &str,
    authored: &[WaveId],
) -> StoreResult<WaveBootstrapInputs> {
    let mut query = conn.prepare(WAVE_BOOTSTRAP_SELECT)?;
    let waves = query
        .query_map(params![repo, serde_json::to_string(authored)?], |row| {
            Ok(WaveBootstrapRow {
                id: row.get(0)?,
                repo: row.get(1)?,
                name: row.get(2)?,
                parent: row.get(3)?,
                retired_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(WaveBootstrapInputs {
        repository: super::repositories::registration_in(conn, repo)?,
        waves,
    })
}

impl SqliteStore {
    pub fn wave_bootstrap_inputs(
        &self,
        repo: &str,
        authored: &[WaveId],
    ) -> StoreResult<WaveBootstrapInputs> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        bootstrap_inputs_in(&snapshot, repo, authored)
    }

    pub fn commit_wave_bootstrap(
        &self,
        repo: &str,
        authored: &[WaveId],
        inputs: &WaveBootstrapInputs,
        registration: Option<(&RepositoryRegistration, &RepositoryId)>,
        additions: &[WaveBootstrapRow],
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if bootstrap_inputs_in(&tx, repo, authored)? != *inputs {
            return Ok(false);
        }
        if let Some((saved, id)) = registration {
            if !super::repositories::register_in(&tx, repo, saved, id, false)? {
                return Ok(false);
            }
        }
        if !additions.is_empty() {
            let occupied: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM waves WHERE id IN (SELECT value FROM json_each(?1)))",
                [serde_json::to_string(
                    &additions.iter().map(|wave| &wave.id).collect::<Vec<_>>(),
                )?],
                |row| row.get(0),
            )?;
            if occupied {
                return Ok(false);
            }
        }
        for wave in additions {
            planning_write::create(
                &tx,
                repo,
                &crate::work::edit::WorkEdit::for_target(
                    wave.id.as_str(),
                    &[
                        Edit::WaveName(wave.name.clone()),
                        Edit::WaveParent(wave.parent.as_ref().map(ToString::to_string)),
                    ],
                ),
                crate::sqlite::planning_write::WriteOrigin::Local,
            )?;
            // A Wave is made with its one Project.
            super::children::wave_project_in(&tx, &wave.id)?;
        }
        tx.commit()?;
        Ok(true)
    }

    /// Retire Waves together through the planning writer, so the hub
    /// carries the removal. Rows, Projects, Tasks and Sessions stay readable;
    /// an already retired Wave keeps its original retirement.
    pub fn retire_waves(&self, waves: &[WaveId], reason: &str) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let at = crate::rows::now_unix();
        for wave in waves {
            let live: bool = tx.query_row(
                "SELECT retired_at IS NULL FROM waves WHERE id=?1",
                [wave],
                |row| row.get(0),
            )?;
            if !live {
                continue;
            }
            planning_write::local(
                &tx,
                &crate::work::edit::WorkEdit::for_target(
                    wave.as_str(),
                    &[Edit::WaveRetired(Some(at))],
                ),
            )?;
            tx.execute(
                "UPDATE waves SET retirement_reason=?2, work_state='abandoned',
                     work_terminal_at=COALESCE(work_terminal_at, ?3)
                 WHERE id=?1",
                params![wave, reason, at],
            )?;
            // A removed Wave's Project never reaches the tracker, nor do its unsent changes.
            super::tracker_actions::retire_wave_in(&tx, wave)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The most recently retired Wave at this address, if any.
    pub fn retired_wave_at(&self, locator: &WaveLocator) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason, slug
             FROM wave_addresses
             WHERE repo=?1 AND slug=?2 AND retired_at IS NOT NULL
             ORDER BY retired_at DESC LIMIT 1",
            params![locator.repo().to_string(), locator.slug()],
            |row| Ok(crate::rows::map_wave_row(row)),
        )
        .optional()?
        .transpose()
    }

    /// Wave conversations (not Task Sessions) recorded for these Waves.
    pub fn wave_session_count(&self, waves: &[WaveId]) -> StoreResult<usize> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut count = 0usize;
        for wave in waves {
            count += conn.query_row(
                "SELECT COUNT(*) FROM agent_sessions WHERE wave_id=?1 AND task_id IS NULL",
                [wave],
                |row| row.get::<_, i64>(0),
            )? as usize;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn released_waves_upgrade_and_removal_needs_no_successor() {
        use crate::migration_runner::{apply_before_migration, apply_pending_migrations};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "wave_removal",
        );
        conn.execute_batch(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/migrations/wave_removal_released.sql"
        )))
        .unwrap();
        apply_pending_migrations(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
        );

        let old: (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT superseded_by_wave_id, retirement_reason FROM waves WHERE id='wave-old'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(old, (Some("wave-live".into()), Some("relocated".into())));
        let addresses: Vec<String> = conn
            .prepare("SELECT slug FROM wave_addresses WHERE retired_at IS NULL ORDER BY slug")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(addresses, ["product", "product/desktop"]);

        conn.execute("UPDATE waves SET retired_at=9 WHERE id='wave-child'", [])
            .unwrap();
        assert!(
            conn.execute(
                "UPDATE waves SET retirement_reason='removed' WHERE id='wave-live'",
                []
            )
            .is_err(),
            "a live Wave carries no retirement reason"
        );
    }

    #[test]
    fn bootstrap_reads_use_indexes_beside_ten_thousand_tasks_and_waves() {
        use super::WAVE_BOOTSTRAP_SELECT;
        use crate::query_plans::{growing_scans, statements};

        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let history = crate::test_fixture::repository(&store, "/history");
        {
            let conn = store.conn.lock().unwrap();
            conn.execute_batch(&format!(
                "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000)
                 INSERT INTO tasks(id,repository_id,issue_identifier,created_at)
                 SELECT printf('task_%032x',x),'{history}',printf('H-%d',x),1 FROM n;
                 INSERT INTO task_checkouts(task_id,worktree,branch)
                 SELECT id,'/history/' || id,'main' FROM tasks;
                 WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000)
                 INSERT INTO waves(id,repo,name,created_at)
                 SELECT printf('%08x-0000-4000-8000-000000000000',x),'/history',printf('w%d',x),1 FROM n;"
            )).unwrap();
        }
        let traced = statements(&store, || {
            assert!(store
                .wave_bootstrap_inputs("/repo", &[])
                .unwrap()
                .waves
                .is_empty());
        });
        let scans = growing_scans(&store, &traced);
        assert!(scans.is_empty(), "{}", scans.join("\n"));
        let conn = store.conn.lock().unwrap();
        let mut query = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {WAVE_BOOTSTRAP_SELECT}"))
            .unwrap();
        let plans = query
            .query_map(rusqlite::params!["/repo", "[]"], |row| {
                row.get::<_, String>(3)
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            !plans.iter().any(|step| step.starts_with("SCAN waves")),
            "{plans:?}"
        );
        assert!(
            plans
                .iter()
                .any(|step| step.contains("idx_waves_active_locator")),
            "{plans:?}"
        );
    }
}
