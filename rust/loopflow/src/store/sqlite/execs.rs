use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::exec::{AgentCaller, SessionDriver};
use crate::id::ExecId;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

pub(super) fn agent_work_in(
    conn: &rusqlite::Connection,
    exec: &ExecId,
) -> StoreResult<Option<crate::session::RunWork>> {
    let row = conn.query_row(
        "SELECT e.caller_provider_generation,s.provider_generation,s.task_id,s.wave_id
         FROM execs e JOIN agent_sessions s ON s.id=e.caller_session_id WHERE e.id=?1 AND e.via_agent=1",
        [exec], |row| Ok((row.get::<_, i64>(0)?,row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,row.get::<_, Option<String>>(3)?)),
    ).optional()?;
    let Some((caller_generation, generation, task, wave)) = row else {
        return Ok(None);
    };
    if caller_generation != generation {
        return Err(StoreError::InvalidAuthority(
            "calling provider was replaced".into(),
        ));
    }
    Ok(Some(crate::session::RunWork {
        task_id: task
            .as_deref()
            .map(crate::durable::TaskId::parse)
            .transpose()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        wave_id: wave
            .as_deref()
            .map(crate::id::WaveId::parse)
            .transpose()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        source: crate::session::WorkSource::Inherited,
    }))
}

fn driver_in(conn: &rusqlite::Connection, session: &str) -> StoreResult<Option<SessionDriver>> {
    let row = conn
        .query_row(
            "SELECT driver_exec_id,driver_generation,provider_generation,provider_exec_id
         FROM agent_sessions WHERE id=?1",
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
    pub fn agent_work(&self, exec: &ExecId) -> StoreResult<Option<crate::session::RunWork>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        agent_work_in(&conn, exec)
    }
    pub(crate) fn make_session_interactive(
        &self,
        session: &str,
        expected: &SessionDriver,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if driver_in(&tx, session)?.as_ref() != Some(expected) || expected.exec_id.is_none() {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        tx.execute(
            "UPDATE agent_sessions SET interactive=1 WHERE id=?1",
            [session],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn session_connection(&self, session: &str) -> StoreResult<Option<(String, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT provider_endpoint,provider_thread FROM agent_sessions WHERE id=?1 AND provider_endpoint IS NOT NULL AND provider_thread IS NOT NULL",
            [session], |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?)
    }

    pub(crate) fn session_thread(&self, session: &str) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT provider_thread FROM agent_sessions WHERE id=?1",
                [session],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    pub(crate) fn session_provider_process(
        &self,
        session: &str,
    ) -> StoreResult<Option<(u32, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT provider_pid,provider_started_at FROM agent_sessions WHERE id=?1
             AND provider_pid IS NOT NULL AND provider_started_at IS NOT NULL",
                [session],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }

    pub(crate) fn record_session_provider_process(
        &self,
        session: &str,
        expected: &SessionDriver,
        pid: u32,
        started_at: i64,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if driver_in(&tx, session)?.as_ref() != Some(expected) || expected.exec_id.is_none() {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        tx.execute(
            "UPDATE agent_sessions SET provider_pid=?2,provider_started_at=?3 WHERE id=?1",
            params![session, pid, started_at],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_session_connection(
        &self,
        session: &str,
        expected: &SessionDriver,
        endpoint: &str,
        thread: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if driver_in(&tx, session)?.as_ref() != Some(expected) || expected.exec_id.is_none() {
            return Err(StoreError::InvalidAuthority(
                "Session driver changed".into(),
            ));
        }
        tx.execute(
            "UPDATE agent_sessions SET provider_endpoint=?2,provider_thread=?3 WHERE id=?1",
            params![session, endpoint, thread],
        )?;
        tx.commit()?;
        Ok(())
    }

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
            "UPDATE agent_sessions SET driver_exec_id=?2,driver_generation=?3,
                provider_generation=?4,provider_exec_id=?5,
                provider_endpoint=CASE WHEN ?6 THEN NULL ELSE provider_endpoint END,
                provider_pid=CASE WHEN ?6 THEN NULL ELSE provider_pid END,
                provider_started_at=CASE WHEN ?6 THEN NULL ELSE provider_started_at END
             WHERE id=?1",
            params![
                session,
                driver.exec_id,
                driver.generation,
                driver.provider_generation,
                driver.provider_exec_id,
                replace_provider
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
            "UPDATE agent_sessions SET driver_exec_id=NULL,driver_generation=?2 WHERE id=?1",
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
