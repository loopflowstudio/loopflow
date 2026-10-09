//! Only the current attachment may close an AgentProcess. A replaced lf
//! invocation may settle its own capture, not stop the new owner's provider.

use crate::process::SessionAttachment;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

pub(crate) fn finish_session_attachment(
    store: &SqliteStore,
    session: &str,
    attachment: &SessionAttachment,
    outcome: &str,
) -> StoreResult<()> {
    let connection = store.session_connection(session)?;
    let process = store.session_provider_process(session)?;
    let provider = store.session(session)?.and_then(|session| session.provider);
    store.finish_session_attachment(session, attachment, outcome, || {
        // Native terminal providers own their own teardown. Codex app-server
        // runs in its own group so another lf invocation can attach live.
        #[cfg(unix)]
        if provider.as_deref() == Some("codex") {
            if let (Some((endpoint, thread)), Some((pid, started))) = (connection, process) {
                let serving = Some((endpoint.as_str(), thread.as_str()));
                crate::harness::codex_connection::close_agent_process(serving, pid, started)
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

/// End the AgentProcess a provably dead lf invocation left behind. The Session
/// lock excludes takeover while the recorded PID/birth is checked and signalled.
/// Failed termination leaves the Session unchanged and refuses replacement.
pub(super) fn end_abandoned_agent_process(
    store: &SqliteStore,
    session: &str,
    dead: &SessionAttachment,
) -> StoreResult<()> {
    store.with_session_attachment(session, dead, || {
        let Some((pid, started)) = store.session_provider_process(session)? else {
            return Ok(());
        };
        let connection = store.session_connection(session)?;
        let codex = store
            .session(session)?
            .is_some_and(|session| session.provider.as_deref() == Some("codex"));
        // Only Codex answers the inspection that spares a shared AgentProcess.
        let serving = connection
            .as_ref()
            .filter(|_| codex)
            .map(|(endpoint, thread)| (endpoint.as_str(), thread.as_str()));
        crate::harness::codex_connection::close_agent_process(serving, pid, started).map_err(|error| {
            StoreError::InvalidAuthority(format!(
                "Conversation's previous AgentProcess (PID {pid}) is still running and was not ended: {error}"
            ))
        })
    })
}
