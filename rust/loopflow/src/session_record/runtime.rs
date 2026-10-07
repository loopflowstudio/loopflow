//! The current driver closes its runtime on exit. A transferred driver can
//! settle its own capture but cannot close the new owner's provider.

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
        // deliberately outlives its launcher so another driver can take over.
        #[cfg(unix)]
        if provider.as_deref() == Some("codex") {
            if let (Some((endpoint, thread)), Some((pid, started))) = (connection, process) {
                crate::harness::codex_connection::close_engine(&endpoint, &thread, pid, started)
                    .map_err(|error| {
                        StoreError::InvalidData(format!(
                            "close Codex conversation {session}: {error}"
                        ))
                    })?;
                return Ok(true);
            }
        }
        Ok(false)
    })
}
