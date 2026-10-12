use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::SqliteStore;
use crate::store::StoreResult;

impl SqliteStore {
    // The acquisition file lock spans network/ingestion; this transaction only
    // claims freshness. A crash leaves a bounded delay and the old watermark.
    pub(crate) fn begin_linear_team_check(
        &self,
        team: &str,
        now: i64,
    ) -> StoreResult<Option<Option<String>>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let saved: Option<(i64, Option<String>)> = tx
            .query_row(
                "SELECT attempted_at,synced_through FROM linear_team_checks WHERE team_id=?1",
                [team],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if saved.as_ref().is_some_and(|(last, _)| now - last < 10) {
            return Ok(None);
        }
        tx.execute(
            "INSERT INTO linear_team_checks(team_id,attempted_at) VALUES(?1,?2)
             ON CONFLICT(team_id) DO UPDATE SET attempted_at=excluded.attempted_at",
            params![team, now],
        )?;
        tx.commit()?;
        Ok(Some(saved.and_then(|(_, through)| through)))
    }

    pub(crate) fn finish_linear_team_check(&self, team: &str, through: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE linear_team_checks SET synced_through=?2 WHERE team_id=?1",
            params![team, through],
        )?;
        Ok(())
    }
}
