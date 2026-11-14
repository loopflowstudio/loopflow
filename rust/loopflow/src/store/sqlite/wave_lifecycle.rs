//! Machine-local receipts for effects outside the planning transaction.

use rusqlite::params;

use super::SqliteStore;
use crate::ops::wave_lifecycle::recovery::Operation;
use crate::store::StoreResult;

impl SqliteStore {
    pub(crate) fn wave_operations(&self, repo: &str, kind: &str) -> StoreResult<Vec<Operation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT operation_json FROM wave_lifecycle_operations
             WHERE repo=?1 AND kind=?2 ORDER BY rowid DESC",
        )?;
        let rows = statement.query_map(params![repo, kind], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }

    pub(crate) fn save_wave_operation(&self, operation: &Operation) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO wave_lifecycle_operations
             (id,repo,wave_id,kind,operation_json)
             VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(id) DO UPDATE SET operation_json=excluded.operation_json",
            params![
                operation.id,
                operation.repo,
                operation.waves[0].id,
                operation.kind,
                serde_json::to_string(operation)?
            ],
        )?;
        Ok(())
    }
}
