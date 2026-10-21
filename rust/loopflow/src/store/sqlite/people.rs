use rusqlite::params;

use crate::person::PersonRecord;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

impl SqliteStore {
    /// Replace the cache with everyone `origin` holds now.
    pub fn replace_people(&self, people: &[PersonRecord]) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM people", [])?;
        let fetched_at = now_unix();
        for person in people {
            let accounts = serde_json::to_string(&person.accounts)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            tx.execute(
                "INSERT INTO people (key, name, accounts, updated_at, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    person.key,
                    person.name,
                    accounts,
                    person.updated_at,
                    fetched_at
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn people(&self) -> StoreResult<Vec<PersonRecord>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt =
            conn.prepare("SELECT key, name, accounts, updated_at FROM people ORDER BY key")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (key, name, accounts, updated_at) = row?;
            Ok(PersonRecord {
                key,
                name,
                accounts: serde_json::from_str(&accounts)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                updated_at,
            })
        })
        .collect()
    }
}
