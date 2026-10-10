//! The current driver closes its runtime on exit. A transferred driver can
//! settle its own capture but cannot close the new owner's provider. A driver
//! that died without closing leaves its engine to whoever replaces it.

use crate::process::SessionDriver;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

pub(crate) fn finish_session_driver(
    store: &SqliteStore,
    session: &str,
    driver: &SessionDriver,
    outcome: &str,
) -> StoreResult<()> {
    let connection = store.session_connection(session)?;
    let process = store.session_provider_process(session)?;
    let provider = store.session(session)?.and_then(|session| session.provider);
    store.finish_session_driver(session, driver, outcome, || {
        // Native terminal providers own their own teardown. Codex app-server
        // runs in its own group so a connecting driver can take it over live.
        #[cfg(unix)]
        if provider.as_deref() == Some("codex") {
            if let (Some((endpoint, thread)), Some((pid, started))) = (connection, process) {
                let serving = Some((endpoint.as_str(), &thread));
                crate::harness::codex_connection::close_engine(serving, pid, started).map_err(
                    |error| {
                        StoreError::InvalidData(format!(
                            "close Codex conversation {session}: {error}"
                        ))
                    },
                )?;
                return Ok(true);
            }
        }
        Ok(false)
    })
}

/// End the engine a provably dead driver left behind, so its replacement never
/// runs beside it. Signals only the recorded PID and start time, under the
/// Session driver lock: a concurrent connect either holds the engine already,
/// which fails this, or finds it gone. An engine that cannot be ended refuses
/// the replacement and leaves the Session as it was.
pub(super) fn end_abandoned_engine(
    store: &SqliteStore,
    session: &str,
    dead: &SessionDriver,
) -> StoreResult<()> {
    store.with_session_driver(session, dead, || {
        let Some((pid, started)) = store.session_provider_process(session)? else {
            return Ok(());
        };
        let connection = store.session_connection(session)?;
        let codex = store
            .session(session)?
            .is_some_and(|session| session.provider.as_deref() == Some("codex"));
        // Only Codex answers the inspection that spares a shared engine.
        let serving = connection
            .as_ref()
            .filter(|_| codex)
            .map(|(endpoint, thread)| (endpoint.as_str(), thread));
        crate::harness::codex_connection::close_engine(serving, pid, started).map_err(|error| {
            StoreError::InvalidAuthority(format!(
                "Conversation's previous engine (process {pid}) is still running and was not ended: {error}"
            ))
        })
    })
}
