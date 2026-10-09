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
    store.finish_session_attachment(session, attachment, outcome, || {
        close_session_agent_process(store, session)
    })
}

/// Called with the exact attachment locked, never with SQLite held across I/O.
/// Native providers settle through their launcher; only Codex owns a detached
/// app-server whose connection can be relinquished before invocation settlement.
pub(super) fn close_session_agent_process(store: &SqliteStore, session: &str) -> StoreResult<bool> {
    let Some(attachment) = store.session_attachment(session)? else {
        return Ok(false);
    };
    let agents = store.agent_processes()?;
    let Some(agent) = agents
        .iter()
        .find(|agent| agent.process.lfid == attachment.agent_process_lfid)
    else {
        return Ok(false);
    };
    let Some((pid, started)) = agent.process.pid.zip(agent.process.os_started_at) else {
        return Ok(false);
    };
    match crate::journal::process_identity_evidence(pid, started) {
        crate::journal::ProcessIdentityEvidence::Dead => return Ok(true),
        crate::journal::ProcessIdentityEvidence::Unknown => {
            return Err(StoreError::InvalidAuthority(
                "AgentProcess OS identity is unavailable".into(),
            ))
        }
        crate::journal::ProcessIdentityEvidence::Live => {}
    }
    #[cfg(unix)]
    if agent.provider.as_deref() == Some("codex") && !agent.interactive {
        if let Some((endpoint, thread)) = store.session_connection(session)? {
            let owners = agents
                .iter()
                .filter(|agent| {
                    agent.process.pid == Some(pid) && agent.process.os_started_at == Some(started)
                })
                .count();
            if owners > 1 {
                return Err(StoreError::InvalidAuthority(
                    "AgentProcess OS identity has multiple owners".into(),
                ));
            }
            crate::harness::codex_connection::close_agent_process(
                Some((&endpoint, &thread)),
                pid,
                started,
            )
            .map_err(|error| {
                StoreError::InvalidData(format!("close Codex conversation {session}: {error}"))
            })?;
            return Ok(true);
        }
    }
    Ok(false)
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

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::process::CommandExt;

    use crate::id::ProcessLfid;
    use crate::store::sqlite::SqliteStore;

    struct Child(std::process::Child);

    impl Drop for Child {
        fn drop(&mut self) {
            if matches!(self.0.try_wait(), Ok(None)) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    #[test]
    fn invocation_replacement_ends_only_its_exact_throwaway_group() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&database).unwrap();
        store.test_session("retry", "run_00000000000000000000000000000001");
        let sql = rusqlite::Connection::open(&database).unwrap();
        sql.execute(
            "UPDATE agent_sessions SET provider='codex',interactive=0 WHERE id='retry'",
            [],
        )
        .unwrap();
        let parent = ProcessLfid::new();
        sql.execute(
            "INSERT INTO processes(lfid,trace_id,started_at) VALUES(?1,?1,1)",
            [&parent],
        )
        .unwrap();
        let attachment = store
            .claim_session_attachment("retry", None, &parent, true)
            .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60").process_group(0);
        store
            .record_session_provider_launch("retry", &attachment, &command)
            .unwrap();
        let child = Child(command.spawn().unwrap());
        let pid = child.0.id();
        let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
        store
            .record_session_provider_process("retry", &attachment, pid, birth)
            .unwrap();
        store
            .record_session_connection(
                "retry",
                &attachment,
                home.path().join("absent.sock").to_str().unwrap(),
                "retained-thread",
            )
            .unwrap();
        // Later launch settings do not change which kind of process was launched.
        sql.execute(
            "UPDATE agent_sessions SET provider='claude' WHERE id='retry'",
            [],
        )
        .unwrap();
        let next = store
            .replace_session_agent_process("retry", &attachment, Some("retained-thread"), || {
                super::close_session_agent_process(&store, "retry")
            })
            .unwrap();
        assert_ne!(next.agent_process_lfid, attachment.agent_process_lfid);
        assert_eq!(
            crate::journal::process_identity_evidence(pid, birth),
            crate::journal::ProcessIdentityEvidence::Dead
        );
        assert!(store
            .process(&attachment.agent_process_lfid)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());
        assert!(store
            .process(&next.agent_process_lfid)
            .unwrap()
            .unwrap()
            .pid
            .is_none());
        assert_eq!(
            store.session_thread("retry").unwrap().as_deref(),
            Some("retained-thread")
        );
    }
}
