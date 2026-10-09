//! Engine identities recorded on Sessions, read and settled by the orphan
//! reaper in `harness::engine_orphans`. The reaper decides from OS evidence;
//! this module only re-checks the row and records the outcome under the
//! Session driver lock, so a takeover cannot race the signal.

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::id::ProcessLfid;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

/// One Session's recorded engine: the OS identity of the provider process and
/// the Process currently driving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordedEngine {
    pub session: String,
    pub provider: Option<String>,
    pub interactive: bool,
    pub pid: u32,
    pub started_at: i64,
    pub driver: Option<ProcessLfid>,
}

impl SqliteStore {
    /// Every Session naming an engine that at least one Session still drives.
    /// A finished driver clears itself, so closed history stays out of the scan.
    pub(crate) fn recorded_engines(&self) -> StoreResult<Vec<RecordedEngine>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id,provider,interactive,provider_pid,provider_started_at,driver_process_lfid
             FROM agent_sessions
             WHERE (provider_pid,provider_started_at) IN (
                SELECT provider_pid,provider_started_at FROM agent_sessions
                WHERE driver_process_lfid IS NOT NULL AND provider_pid IS NOT NULL
                    AND provider_started_at IS NOT NULL)
             ORDER BY id",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, u32>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(session, provider, interactive, pid, started_at, driver)| {
                    Ok(RecordedEngine {
                        session,
                        provider,
                        interactive,
                        pid,
                        started_at,
                        driver: driver
                            .as_deref()
                            .map(ProcessLfid::parse)
                            .transpose()
                            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                    })
                },
            )
            .collect()
    }

    /// Terminate one orphaned engine and record why it went away. `terminate`
    /// runs only while the row still names this exact engine and driver, and
    /// the Session is settled only when it succeeds. Returns false when the
    /// row changed since `engine` was read, leaving everything untouched.
    ///
    /// The exit is recorded as the provider generation's `exited` receipt, the
    /// same confirmed exit recovery already reads before replacing an engine.
    pub(crate) fn reap_session_engine(
        &self,
        engine: &RecordedEngine,
        terminate: impl FnOnce() -> StoreResult<()>,
    ) -> StoreResult<bool> {
        let _dispatch = self.lock_session_driver(&engine.session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let generation: Option<i64> = tx
            .query_row(
                "SELECT provider_generation FROM agent_sessions WHERE id=?1
                 AND provider_pid=?2 AND provider_started_at=?3
                 AND driver_process_lfid IS ?4 AND interactive=?5",
                params![
                    engine.session,
                    engine.pid,
                    engine.started_at,
                    engine.driver,
                    engine.interactive
                ],
                |row| row.get(0),
            )
            .optional()?;
        let Some(generation) = generation else {
            return Ok(false);
        };
        terminate()?;
        let payload = serde_json::json!({
            "type": "provider_launch", "phase": "exited", "provider_generation": generation,
            "reaped": {
                "reason": "driver exited without closing its engine",
                "pid": engine.pid, "started_at": engine.started_at,
                "driver_process_lfid": engine.driver,
                "reaped_by": crate::journal::current_process_lfid(),
            },
        });
        tx.execute(
            "INSERT OR IGNORE INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
             SELECT id,'observed',?2,?3,?4,current_capture FROM agent_sessions WHERE id=?1",
            params![
                engine.session,
                format!("provider:{generation}:exited"),
                time::OffsetDateTime::now_utc().unix_timestamp(),
                payload.to_string()
            ],
        )?;
        tx.execute(
            "UPDATE agent_sessions SET provider_pid=NULL,provider_started_at=NULL,
                provider_endpoint=NULL WHERE id=?1",
            [&engine.session],
        )?;
        tx.commit()?;
        Ok(true)
    }
}
