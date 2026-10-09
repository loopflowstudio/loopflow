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
                (&endpoint, &thread),
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

    fn resume_fixture() -> (
        tempfile::TempDir,
        SqliteStore,
        crate::process::SessionAttachment,
    ) {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("resume", &crate::session_record::new_artifact_key());
        let sql = rusqlite::Connection::open(home.path().join("store.db")).unwrap();
        sql.execute(
            "UPDATE agent_sessions SET provider='codex',interactive=0",
            [],
        )
        .unwrap();
        let attached = store
            .claim_session_attachment("resume", None, &ProcessLfid::new(), true)
            .unwrap();
        let detached = store
            .release_session_attachment("resume", &attached)
            .unwrap();
        (home, store, detached)
    }

    #[test]
    fn resume_detached_live_agent_uses_recorded_settings_and_preserves_history() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (home, store, detached) = resume_fixture();
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &ProcessLfid::new(), false)
            .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60").process_group(0);
        let child = Child(command.spawn().unwrap());
        let pid = child.0.id();
        let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
        store
            .record_session_provider_process("resume", &attached, pid, birth)
            .unwrap();
        store
            .record_session_connection(
                "resume",
                &attached,
                home.path().join("absent.sock").to_str().unwrap(),
                "saved-thread",
            )
            .unwrap();
        store
            .release_session_attachment("resume", &attached)
            .unwrap();
        rusqlite::Connection::open(home.path().join("store.db"))
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider='claude',interactive=1",
                [],
            )
            .unwrap();
        let next = crate::session_record::resume_session_agent_process(
            &store,
            "resume",
            &ProcessLfid::new(),
        )
        .unwrap();
        assert_ne!(next.agent_process_lfid, attached.agent_process_lfid);
        assert_eq!(
            crate::journal::process_identity_evidence(pid, birth),
            crate::journal::ProcessIdentityEvidence::Dead
        );
        let ended = store
            .process(&attached.agent_process_lfid)
            .unwrap()
            .unwrap();
        assert!(ended.completed_at.is_some());
        assert!(ended.outcome.is_none());
        assert_eq!(ended.pid, Some(pid));
        assert_eq!(
            store.session_thread("resume").unwrap().as_deref(),
            Some("saved-thread")
        );
        assert!(store.session_connection("resume").unwrap().is_none());
    }

    #[test]
    fn resume_unknown_spawn_preserves_record_attachment_and_input() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (_home, store, detached) = resume_fixture();
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &ProcessLfid::new(), false)
            .unwrap();
        store
            .record_session_provider_launch(
                "resume",
                &attached,
                &std::process::Command::new("fixture"),
            )
            .unwrap();
        let detached = store
            .release_session_attachment("resume", &attached)
            .unwrap();
        let before = store.process(&detached.agent_process_lfid).unwrap();
        let session = store.session("resume").unwrap().unwrap();
        let mut next = session.clone();
        next.artifact_key = crate::session_record::new_artifact_key();
        next.input_published = false;
        assert!(store
            .claim_session_input(next, Some(&detached), &ProcessLfid::new(), || {
                super::close_session_agent_process(&store, "resume")
            })
            .is_err());
        assert!(crate::session_record::resume_session_agent_process(
            &store,
            "resume",
            &ProcessLfid::new()
        )
        .is_err());
        assert_eq!(store.process(&detached.agent_process_lfid).unwrap(), before);
        assert_eq!(store.session_attachment("resume").unwrap(), Some(detached));
        assert_eq!(store.session("resume").unwrap(), Some(session));
    }

    #[test]
    fn resume_duplicate_live_identity_cannot_signal_either_session() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (home, store, detached) = resume_fixture();
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &ProcessLfid::new(), false)
            .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60").process_group(0);
        let mut child = Child(command.spawn().unwrap());
        let pid = child.0.id();
        let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
        store
            .record_session_provider_process("resume", &attached, pid, birth)
            .unwrap();
        store
            .record_session_connection(
                "resume",
                &attached,
                home.path().join("absent.sock").to_str().unwrap(),
                "saved",
            )
            .unwrap();
        let detached = store
            .release_session_attachment("resume", &attached)
            .unwrap();
        store.test_session("duplicate", &crate::session_record::new_artifact_key());
        let duplicate = store
            .claim_session_attachment("duplicate", None, &ProcessLfid::new(), true)
            .unwrap();
        store
            .record_session_provider_process("duplicate", &duplicate, pid, birth)
            .unwrap();
        let error = crate::session_record::resume_session_agent_process(
            &store,
            "resume",
            &ProcessLfid::new(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("multiple owners"), "{error}");
        assert!(child.0.try_wait().unwrap().is_none());
        assert_eq!(store.session_attachment("resume").unwrap(), Some(detached));
        assert_eq!(store.agent_processes().unwrap().len(), 2);
    }

    #[test]
    fn resume_zombie_is_dead_without_signaling_or_inventing_success() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (_home, store, detached) = resume_fixture();
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &ProcessLfid::new(), false)
            .unwrap();
        let mut child = Child(
            std::process::Command::new("/bin/sleep")
                .env_clear()
                .arg("60")
                .spawn()
                .unwrap(),
        );
        let pid = child.0.id();
        let birth = crate::journal::process_started_at(pid).unwrap().unwrap();
        store
            .record_session_provider_process("resume", &attached, pid, birth)
            .unwrap();
        store
            .release_session_attachment("resume", &attached)
            .unwrap();
        child.0.kill().unwrap();
        // Observe this fixture's zombie without reaping it through Child::wait.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while crate::journal::process_identity_evidence(pid, birth)
            != crate::journal::ProcessIdentityEvidence::Dead
        {
            assert!(
                std::time::Instant::now() < deadline,
                "fixture child did not exit"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let next = crate::session_record::resume_session_agent_process(
            &store,
            "resume",
            &ProcessLfid::new(),
        )
        .unwrap();
        assert_ne!(next.agent_process_lfid, attached.agent_process_lfid);
        let ended = store
            .process(&attached.agent_process_lfid)
            .unwrap()
            .unwrap();
        assert!(ended.completed_at.is_some());
        assert!(ended.outcome.is_none());
    }

    #[test]
    fn resume_fences_takeover_until_settlement_and_claim_commit() {
        let _ambient = crate::test_ambient::EnvGuard::new();
        let (_home, store, detached) = resume_fixture();
        let attached = store
            .claim_session_attachment("resume", Some(&detached), &ProcessLfid::new(), false)
            .unwrap();
        store
            .record_session_provider_launch(
                "resume",
                &attached,
                &std::process::Command::new("fixture"),
            )
            .unwrap();
        let detached = store
            .release_session_attachment("resume", &attached)
            .unwrap();
        let (started, starting) = std::sync::mpsc::channel();
        let (finished, finishing) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let next = store
                .resume_session_attachment("resume", Some(&detached), &ProcessLfid::new(), || {
                    let store = &store;
                    let expected = &detached;
                    scope.spawn(move || {
                        started.send(()).unwrap();
                        let result = store.claim_session_attachment(
                            "resume",
                            Some(expected),
                            &ProcessLfid::new(),
                            false,
                        );
                        finished.send(result).unwrap();
                    });
                    starting.recv().unwrap();
                    assert!(matches!(
                        finishing.recv_timeout(std::time::Duration::from_millis(100)),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                    ));
                    // Other SQLite reads/writes remain available during provider I/O.
                    assert_eq!(
                        store.session_attachment("resume").unwrap(),
                        Some(detached.clone())
                    );
                    Ok(true)
                })
                .unwrap();
            assert!(finishing
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .is_err());
            assert_eq!(
                store.session_attachment("resume").unwrap(),
                Some(next.clone())
            );
            assert_ne!(next.agent_process_lfid, detached.agent_process_lfid);
            assert!(store
                .process(&detached.agent_process_lfid)
                .unwrap()
                .unwrap()
                .completed_at
                .is_some());
        });
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
