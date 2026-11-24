use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::SqliteStore;
use crate::StoreResult;

/// Last attempt for a planning scope, and its tracker watermark when it has one.
/// Linear keeps its released team keys. Asana projects and carrier polls use prefixes.
fn saved_check(
    conn: &rusqlite::Connection,
    scope: &str,
) -> StoreResult<Option<(i64, Option<String>)>> {
    Ok(conn
        .query_row(
            "SELECT attempted_at,synced_through FROM planning_scope_checks WHERE scope=?1",
            [scope],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

impl SqliteStore {
    /// Paths with linked Tasks, including records under another Machine's
    /// repository alias. Resolve the tracker scope once per path.
    pub fn tracked_repositories(&self) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT DISTINCT r.repo FROM repository_plans r WHERE EXISTS (
                SELECT 1 FROM tasks t WHERE t.repository_id=r.id AND t.external_issue_id IS NOT NULL)",
        )?;
        let rows = query.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Open and finished issue IDs for a tracker's delta filter.
    pub fn tracked_issue_ids(&self, repo: &str) -> StoreResult<(Vec<String>, Vec<String>)> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT t.external_issue_id,{} FROM repository_plans r
             CROSS JOIN tasks t ON t.repository_id=r.id WHERE r.repo=?1
             AND t.external_issue_id IS NOT NULL AND {}",
            super::durable::task_open_sql("t"),
            super::children::TASK_VISIBLE
        ))?;
        let mut open = Vec::new();
        let mut finished = Vec::new();
        let mut rows = query.query([repo])?;
        while let Some(row) = rows.next()? {
            let ids = if row.get::<_, bool>(1)? {
                &mut open
            } else {
                &mut finished
            };
            ids.push(row.get(0)?);
        }
        Ok((open, finished))
    }

    pub fn last_scope_attempt(&self, scope: &str) -> StoreResult<Option<i64>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(saved_check(&conn, scope)?.map(|(at, _)| at))
    }

    // The acquisition file lock spans network/ingestion; this transaction only
    // claims freshness. A crash leaves a bounded delay and the old watermark.
    pub fn begin_scope_check(
        &self,
        scope: &str,
        now: i64,
        fresh_for: i64,
    ) -> StoreResult<Option<Option<String>>> {
        let fresh = |saved: &Option<(i64, Option<String>)>| {
            saved.as_ref().is_some_and(|(at, _)| now - at < fresh_for)
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        // Nearly every check finds the scope fresh; that answer takes no write lock.
        if fresh(&saved_check(&conn, scope)?) {
            return Ok(None);
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let saved = saved_check(&tx, scope)?;
        if fresh(&saved) {
            return Ok(None);
        }
        tx.execute(
            "INSERT INTO planning_scope_checks(scope,attempted_at) VALUES(?1,?2)
             ON CONFLICT(scope) DO UPDATE SET attempted_at=excluded.attempted_at",
            params![scope, now],
        )?;
        tx.commit()?;
        Ok(Some(saved.and_then(|(_, through)| through)))
    }

    /// Whether the scope's last finished delta is within `fresh_for`
    /// seconds of `now`.
    pub fn scope_current(&self, scope: &str, now: i64, fresh_for: i64) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let through: Option<String> = conn
            .query_row(
                "SELECT synced_through FROM planning_scope_checks WHERE scope=?1",
                [scope],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        Ok(through
            .and_then(|through| {
                time::OffsetDateTime::parse(
                    &through,
                    &time::format_description::well_known::Rfc3339,
                )
                .ok()
            })
            .is_some_and(|through| now - through.unix_timestamp() < fresh_for))
    }

    pub fn finish_scope_check(&self, scope: &str, through: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE planning_scope_checks SET synced_through=?2 WHERE scope=?1",
            params![scope, through],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn finished_team_delta_answers_reads_inside_its_window() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        // 2026-10-29T00:00:00Z
        let start = 1_793_232_000;
        assert_eq!(
            store.begin_scope_check("team", start, 10).unwrap(),
            Some(None)
        );
        // Begun but unfinished: nothing local answers yet.
        assert!(!store.scope_current("team", start, 10).unwrap());
        store
            .finish_scope_check("team", "2026-10-29T00:00:00Z")
            .unwrap();
        assert!(store.scope_current("team", start + 9, 10).unwrap());
        assert!(!store.scope_current("team", start + 10, 10).unwrap());
        // A tighter budget stretches the window over the same delta.
        assert!(store.scope_current("team", start + 90, 100).unwrap());
        assert_eq!(
            store.begin_scope_check("team", start + 90, 100).unwrap(),
            None
        );
    }

    #[test]
    fn scope_migration_keeps_attempts_and_watermarks() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "planning_scope_checks",
        );
        conn.execute_batch(
            "INSERT INTO linear_team_checks(team_id,attempted_at,synced_through) VALUES
            ('team',10,'through'),('asana:team/project',20,'gid:through'),('plan:/repo',30,NULL)",
        )
        .unwrap();
        conn.execute_batch(&migration_section_sql("planning_scope_checks"))
            .unwrap();
        for (scope, old) in [
            ("team", (10, Some("through".to_owned()))),
            ("asana:team/project", (20, Some("gid:through".to_owned()))),
            ("plan:/repo", (30, None)),
        ] {
            assert_eq!(super::saved_check(&conn, scope).unwrap(), Some(old));
        }
    }

    #[test]
    fn linear_on_demand_migration_retains_released_work() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "linear_team_checks",
        );
        conn.execute_batch(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/migrations/linear_team_checks_released.sql"
        )))
        .unwrap();
        conn.execute_batch(&migration_section_sql("linear_team_checks"))
            .unwrap();
        let work: (String, String) = conn
            .query_row(
                "SELECT id,repo FROM waves WHERE name='fixture'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            work,
            (
                "11111111-1111-4111-8111-111111111111".into(),
                "/repo".into()
            )
        );
        conn.execute(
            "INSERT INTO linear_team_checks(team_id,attempted_at) VALUES('team',42)",
            [],
        )
        .unwrap();
        let stamp: (i64, Option<String>) = conn
            .query_row(
                "SELECT attempted_at,synced_through FROM linear_team_checks WHERE team_id='team'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(stamp, (42, None));
        assert!(conn
            .execute(
                "INSERT INTO linear_team_checks(team_id,attempted_at) VALUES('team',43)",
                []
            )
            .is_err());
    }
}
