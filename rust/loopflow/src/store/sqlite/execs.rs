use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::exec::{AgentCaller, SessionDriver};
use crate::id::ExecId;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

fn driver_in(conn: &rusqlite::Connection, session: &str) -> StoreResult<Option<SessionDriver>> {
    let row = conn
        .query_row(
            "SELECT driver_exec_id,driver_generation,provider_generation,provider_exec_id
         FROM sessions WHERE id=?1",
            [session],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    if row.1 == 0 {
        return Ok(None);
    }
    let parse =
        |value: &str| ExecId::parse(value).map_err(|e| StoreError::InvalidData(e.to_string()));
    Ok(Some(SessionDriver {
        exec_id: row.0.as_deref().map(parse).transpose()?,
        generation: row.1,
        provider_generation: row.2,
        provider_exec_id: parse(row.3.as_deref().ok_or_else(|| {
            StoreError::InvalidData("Session driver has no provider origin".into())
        })?)?,
    }))
}

impl SqliteStore {
    /// Serialize native dispatch with driver transfer. The bounded transport
    /// write finishes before a replacement can acquire the same Session.
    pub(crate) fn with_session_driver<T>(
        &self,
        session: &str,
        expected: &SessionDriver,
        write: impl FnOnce() -> StoreResult<T>,
    ) -> StoreResult<T> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if expected.exec_id.is_none() || driver_in(&tx, session)?.as_ref() != Some(expected) {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        let result = write()?;
        tx.commit()?;
        Ok(result)
    }

    pub fn session_driver(&self, session: &str) -> StoreResult<Option<SessionDriver>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        driver_in(&conn, session)
    }

    pub fn claim_session_driver(
        &self,
        session: &str,
        expected: Option<&SessionDriver>,
        exec: &ExecId,
        replace_provider: bool,
    ) -> StoreResult<SessionDriver> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = driver_in(&tx, session)?;
        if current.as_ref() != expected {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        let driver = SessionDriver {
            exec_id: Some(exec.clone()),
            generation: current.as_ref().map_or(1, |value| value.generation + 1),
            provider_generation: current.as_ref().map_or(1, |value| {
                value.provider_generation + i64::from(replace_provider)
            }),
            provider_exec_id: current
                .as_ref()
                .filter(|_| !replace_provider)
                .map_or_else(|| exec.clone(), |value| value.provider_exec_id.clone()),
        };
        tx.execute(
            "UPDATE sessions SET driver_exec_id=?2,driver_generation=?3,
                provider_generation=?4,provider_exec_id=?5 WHERE id=?1",
            params![
                session,
                driver.exec_id,
                driver.generation,
                driver.provider_generation,
                driver.provider_exec_id
            ],
        )?;
        tx.commit()?;
        Ok(driver)
    }

    pub fn release_session_driver(
        &self,
        session: &str,
        expected: &SessionDriver,
    ) -> StoreResult<SessionDriver> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut current = driver_in(&tx, session)?.ok_or(StoreError::NotFound)?;
        if current != *expected {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        current.exec_id = None;
        current.generation += 1;
        tx.execute(
            "UPDATE sessions SET driver_exec_id=NULL,driver_generation=?2 WHERE id=?1",
            params![session, current.generation],
        )?;
        tx.commit()?;
        Ok(current)
    }

    /// Stale providers retain their historical caller, never the new driver.
    pub fn agent_parent(&self, caller: &AgentCaller) -> StoreResult<Option<(ExecId, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let driver = driver_in(&conn, &caller.session_id)?;
        let parent = driver
            .as_ref()
            .filter(|driver| {
                driver.provider_generation == caller.provider_generation
                    && driver.provider_exec_id == caller.origin_exec_id
            })
            .and_then(|driver| driver.exec_id.as_ref())
            .unwrap_or(&caller.origin_exec_id);
        conn.query_row("SELECT trace_id FROM execs WHERE id=?1", [parent], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map(|trace| trace.map(|trace| (parent.clone(), trace)))
        .map_err(StoreError::from)
    }
}
