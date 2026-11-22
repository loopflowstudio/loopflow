//! Coordinate provider lifetime above saved process state and SQL transactions.

use crate::process::{AgentProcessState, AttachingLf, Departure, SessionAttachment};
use crate::session::LfSession;
use crate::store::{StoreError, StoreResult};
use loopflow_model::id::{AgentSessionId, LfProcessId};
use loopflow_storage::sqlite::{PreviousAgentDisposition, SessionTransfer, SqliteStore};

pub fn session_connection(
    store: &SqliteStore,
    session: &str,
) -> StoreResult<Option<(String, AgentSessionId)>> {
    let Some(saved) = store.saved_session_connection(session)? else {
        return Ok(None);
    };
    let agent = crate::process::RecordedProcess {
        ended: saved.completed_at.is_some(),
        pid: saved.pid,
        os_started_at: saved.os_started_at,
        started_at: None,
    };
    match agent.evidence() {
        crate::store::ProcessIdentityEvidence::Live => Ok(Some((saved.endpoint, saved.thread))),
        crate::store::ProcessIdentityEvidence::Dead => Ok(None),
        crate::store::ProcessIdentityEvidence::Unknown => Err(StoreError::InvalidAuthority(
            "AgentProcess OS identity is unavailable; cannot reconnect or replace it".into(),
        )),
    }
}

pub(crate) fn resume_session_attachment(
    store: &SqliteStore,
    session: &str,
    process: &LfProcessId,
    close: impl FnOnce() -> StoreResult<bool>,
) -> StoreResult<SessionAttachment> {
    let caller = crate::store::sessions::session_caller()?;
    let canonical = loopflow_model::id::canonical_session(session);
    let session = canonical.as_str();
    let resume = store.begin_session_resume(session)?;
    let previous = prepare_resume(store, &resume, close)?;
    let start = current_start(process);
    resume.claim_attachment(process, previous, start.as_ref(), &caller)
}

/// Input replacement and attachment admission commit together. The caller
/// retains both checkout admission and attachment fences without holding SQL.
pub(crate) fn claim_session_input(
    store: &SqliteStore,
    next: LfSession,
    expected: Option<&SessionAttachment>,
    process: &LfProcessId,
    close: impl FnOnce() -> StoreResult<bool>,
) -> StoreResult<(LfSession, SessionAttachment)> {
    let caller = crate::store::sessions::session_caller()?;
    let _admission = crate::store::admission::lock_session_checkouts(store, &next)?;
    let resume = store.begin_session_transfer(&next.id, expected)?;
    let previous = prepare_resume(store, &resume, close)?;
    let start = current_start(process);
    resume.claim_input(next, process, previous, start.as_ref(), &caller)
}

pub fn claim_session_attachment(
    store: &SqliteStore,
    session: &str,
    expected: Option<&SessionAttachment>,
    process: &LfProcessId,
) -> StoreResult<SessionAttachment> {
    let caller = crate::store::sessions::session_caller()?;
    let start = current_start(process);
    store.attach_session(session, expected, process, start.as_ref(), &caller)
}

pub(crate) fn prepare_session_agent_process(
    store: &SqliteStore,
    session: &str,
    expected: &SessionAttachment,
) -> StoreResult<SessionAttachment> {
    let start = expected.lf_process_id.as_ref().and_then(current_start);
    store.prepare_agent_process(session, expected, start.as_ref())
}

pub(crate) fn replace_session_agent_process(
    store: &SqliteStore,
    session: &str,
    expected: &SessionAttachment,
    resume_thread: Option<&AgentSessionId>,
    close: impl FnOnce() -> StoreResult<bool>,
) -> StoreResult<SessionAttachment> {
    let caller = crate::store::sessions::session_caller()?;
    let transfer = store.begin_session_transfer(session, Some(expected))?;
    transfer.require_owner()?;
    let (ended, reserved) = transfer.previous_progress()?;
    if !ended && !reserved && !close()? {
        return Err(StoreError::InvalidAuthority(
            "Previous AgentProcess has no observed exit".into(),
        ));
    }
    let start = expected.lf_process_id.as_ref().and_then(current_start);
    transfer.replace_agent(resume_thread, reserved, start.as_ref(), &caller)
}

/// Close and settle the exact attachment under the transfer fence, with no
/// SQLite lock held during provider I/O.
pub fn finish_session_attachment(
    store: &SqliteStore,
    session: &str,
    expected: &SessionAttachment,
    outcome: &str,
    departure: Option<&Departure>,
    close_provider: impl FnOnce() -> StoreResult<bool>,
) -> StoreResult<()> {
    let transfer = store.begin_session_transfer(session, Some(expected))?;
    let closed = close_provider()?;
    transfer.finish(outcome, departure, closed)
}

/// A stale process cannot authorize termination. Retain the Session fence
/// from the saved-image comparison through termination and settlement.
pub(crate) fn settle_agent_process(
    store: &SqliteStore,
    expected: &AgentProcessState,
    terminate: impl FnOnce() -> StoreResult<()>,
) -> StoreResult<bool> {
    let Some(settlement) = store.begin_agent_settlement(expected)? else {
        return Ok(false);
    };
    terminate()?;
    settlement.commit()?;
    Ok(true)
}

/// Observe and close while the saved attachment remains fenced. A live or
/// unknown holder cannot be taken over, and a previous agent with no observed
/// exit is never left running beside the new one.
fn prepare_resume(
    store: &SqliteStore,
    resume: &SessionTransfer<'_>,
    close: impl FnOnce() -> StoreResult<bool>,
) -> StoreResult<PreviousAgentDisposition> {
    if let Some(attachment) = resume.attachment() {
        let holder = store
            .agent_state(&attachment.agent_process_id)?
            .and_then(|agent| agent.holder);
        if let Some(holder) = holder {
            if crate::process::RecordedProcess::from(&holder).evidence()
                != crate::store::ProcessIdentityEvidence::Dead
            {
                return Err(StoreError::InvalidAuthority(
                    "Conversation already has an attached LfProcess; connect to it".into(),
                ));
            }
        }
    }
    let (ended, reserved) = resume.previous_progress()?;
    if ended || reserved {
        return Ok(PreviousAgentDisposition::Inactive);
    }
    if !close()? {
        return Err(StoreError::InvalidAuthority(
            "Previous AgentProcess has no observed exit".into(),
        ));
    }
    Ok(PreviousAgentDisposition::Closed)
}

/// This lf, when `process` is this lf: the holder its attachment records.
fn current_start(process: &LfProcessId) -> Option<AttachingLf> {
    crate::journal::current_holder(process).map(|holder| AttachingLf {
        lf_process_id: process.clone(),
        holder,
        terminal: crate::transport::current().cloned(),
    })
}
