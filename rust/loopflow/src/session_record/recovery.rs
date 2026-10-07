//! Unknown engines may be replaced after an observed restart of their host.
//! A failed Process or a missing PID does not establish this boundary.

use serde::{Deserialize, Serialize};

use crate::process::SessionDriver;
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HostBoot {
    pub host: String,
    pub machine: String,
    pub boot: String,
}

pub(super) fn prepare_after_restart(
    store: &SqliteStore,
    session: &str,
    expected: Option<&SessionDriver>,
) -> StoreResult<bool> {
    let Some(expected) = expected.filter(|driver| driver.process_lfid.is_none()) else {
        return Ok(false);
    };
    let Some(boot) = host_boot() else {
        return Ok(false);
    };
    store.observe_session_recovery_boot(session, expected, &boot)
}

#[cfg(target_os = "macos")]
fn host_boot() -> Option<HostBoot> {
    let mut machine = [0_u8; 16];
    let timeout = libc::timespec {
        tv_sec: 1,
        tv_nsec: 0,
    };
    // SAFETY: both pointers refer to initialized storage of the required size.
    if unsafe { libc::gethostuuid(machine.as_mut_ptr(), &timeout) } != 0 {
        return None;
    }
    let mut boot = [0_u8; 64];
    let mut length = boot.len();
    // SAFETY: the NUL-terminated name and writable buffer remain valid for the
    // call; a null new-value pointer makes this a read-only kernel query.
    if unsafe {
        libc::sysctlbyname(
            c"kern.bootsessionuuid".as_ptr(),
            boot.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || length > boot.len()
    {
        return None;
    }
    let boot = std::str::from_utf8(&boot[..length])
        .ok()?
        .trim_end_matches('\0');
    Some(HostBoot {
        host: gethostname::gethostname().to_str()?.to_owned(),
        machine: uuid::Uuid::from_bytes(machine).to_string(),
        boot: uuid::Uuid::parse_str(boot).ok()?.to_string(),
    })
}

#[cfg(target_os = "linux")]
fn host_boot() -> Option<HostBoot> {
    let machine = std::fs::read_to_string("/etc/machine-id").ok()?;
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    Some(HostBoot {
        host: gethostname::gethostname().to_str()?.to_owned(),
        machine: uuid::Uuid::parse_str(machine.trim()).ok()?.to_string(),
        boot: uuid::Uuid::parse_str(boot.trim()).ok()?.to_string(),
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn host_boot() -> Option<HostBoot> {
    None
}

#[cfg(test)]
mod tests {
    use super::HostBoot;
    use crate::id::ProcessLfid;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn saved_conversation_admission_recovers_after_recorded_restart() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let command = vec!["lf".into(), "skill".into()];
        let capture = crate::journal::with_runtime(ledger.home(), &command, || {
            let capture = crate::session_record::CaptureHandle::begin_at(
                ledger.home(),
                crate::session_record::SessionCaptureSpec {
                    harness: "codex".into(),
                    model: None,
                    surface: "test".into(),
                    cwd: ledger.home().into(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: vec![],
                    flow: crate::session_record::SessionFlowMembership::Independent,
                    work: None,
                },
            )?;
            capture.claim_conversation_driver()?;
            let store = super::super::row_store(&capture.artifact_dir())?;
            let (session, driver) = capture.session_driver().unwrap();
            store.record_session_connection(&session, &driver, "/absent.sock", "native-thread")?;
            capture.finish("failed")?;
            // Model the released runtime's lost process/connection evidence.
            let sql = rusqlite::Connection::open(ledger.home().join("loopflow.db"))?;
            sql.execute(
                "UPDATE agent_sessions SET provider_endpoint=NULL WHERE id=?1",
                [&session],
            )?;
            sql.execute(
                "DELETE FROM session_events WHERE session_id=?1 AND receipt_key LIKE 'provider:%'",
                [&session],
            )?;
            Ok(capture)
        })
        .unwrap();
        let store = super::super::row_store(&capture.artifact_dir()).unwrap();
        let session = store
            .session_for_artifact(&capture.artifact_key())
            .unwrap()
            .unwrap();
        let before = store.session_driver(&session.id).unwrap().unwrap();
        let manifest = super::super::read_manifest(&capture.artifact_dir()).unwrap();
        let retry =
            crate::session_record::CaptureHandle(std::sync::Arc::new(std::sync::Mutex::new(
                super::super::SessionCapture::from_manifest(manifest, capture.artifact_dir()),
            )));
        crate::journal::with_runtime(ledger.home(), &command, || {
            assert!(retry.claim_conversation_driver().is_err());
            assert_eq!(store.session_driver(&session.id)?, Some(before.clone()));
            Ok(())
        })
        .unwrap();
        let boot = super::host_boot().expect("test host supplies an OS boot identity");
        // Substitute only the old kernel observation; production reads it from
        // append-only history and gets the current boot directly from the OS.
        let sql = rusqlite::Connection::open(ledger.home().join("loopflow.db")).unwrap();
        sql.execute(
            "UPDATE session_events SET payload=json_set(payload,'$.host.boot','previous-boot')
            WHERE session_id=?1 AND receipt_key=?2",
            rusqlite::params![
                session.id,
                format!("driver:{}:recovery_boot", before.generation)
            ],
        )
        .unwrap();
        crate::journal::with_runtime(ledger.home(), &command, || {
            retry.claim_conversation_driver()?;
            let (_, current) = retry.session_driver().unwrap();
            assert_eq!(current.provider_generation, before.provider_generation + 1);
            assert_eq!(
                retry.conversation_resume_token()?.as_deref(),
                Some("native-thread")
            );
            assert!(store
                .record_session_provider_process(&session.id, &before, 123, 1)
                .is_err());
            retry.finish("failed")?;
            Ok(())
        })
        .unwrap();
        let history = store.session_history(&session.id, 0, 100).unwrap();
        assert!(history
            .iter()
            .any(|event| event.payload["type"] == "recovered_after_restart"
                && event.payload["host"]["boot"] == boot.boot));
        assert!(history
            .iter()
            .all(|event| event.kind != crate::session::SessionEventKind::Completed));
    }

    #[test]
    fn historical_unknown_engine_requires_same_host_restart_and_exact_driver() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        store.test_session("stranded", &input);
        let sql = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let process = ProcessLfid::new();
        sql.execute(
            "INSERT INTO processes(lfid,trace_id,started_at,completed_at,outcome,error)
             VALUES(?1,'00000000-0000-0000-0000-000000000001',1,2,'failed','Saved conversation thread differs; reconnect with its recorded provider')",
            [process.as_str()],
        ).unwrap();
        sql.execute(
            "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,process_lfid,payload)
            VALUES('stranded','captured','legacy-input',1,?1,'{}')",
            [process.as_str()],
        )
        .unwrap();
        sql.execute(
            "UPDATE agent_sessions SET current_capture=?1 WHERE id='stranded'",
            [sql.last_insert_rowid()],
        )
        .unwrap();
        sql.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
            SELECT 'stranded','observed','legacy-input:manifest.json',1,
            json_object('input_id','legacy-input','source','manifest.json','evidence',
                json_object('schema_version',1,'artifact_key','legacy-input','host','fixture-host')),current_capture
            FROM agent_sessions WHERE id='stranded'", []).unwrap();
        let initial = store
            .claim_session_driver("stranded", None, &process, true)
            .unwrap();
        store
            .record_session_connection("stranded", &initial, "/absent.sock", "saved-native-thread")
            .unwrap();
        // Reproduce the legacy admission: replacement lost process evidence,
        // then a failed attempt released its driver without a spawn receipt.
        let driver = store
            .claim_session_driver("stranded", Some(&initial), &process, true)
            .unwrap();
        let boot = HostBoot {
            host: "fixture-host".into(),
            machine: "machine-a".into(),
            boot: "boot-a".into(),
        };
        assert!(store
            .observe_session_recovery_boot("stranded", &driver, &boot)
            .is_err());
        let released = store.release_session_driver("stranded", &driver).unwrap();
        assert!(!store.session_provider_unstarted("stranded").unwrap());
        assert!(!crate::session_record::conversation_engine_exited(&store, "stranded").unwrap());
        // Retain the released witness encoding across the Process rename.
        let witness = serde_json::json!({
            "type": "recovery_boot", "host": boot,
            "provider_generation": released.provider_generation,
            "provider_exec_id": released.provider_process_lfid,
            "previous": null,
        });
        sql.execute(
            "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
             SELECT id,'observed',?1,1,?2,current_capture FROM agent_sessions WHERE id='stranded'",
            rusqlite::params![format!("driver:{}:recovery_boot", released.generation), witness.to_string()],
        ).unwrap();
        for _ in 0..2 {
            assert!(!store
                .observe_session_recovery_boot("stranded", &released, &boot)
                .unwrap());
        }
        let other_host = HostBoot {
            host: "fixture-host".into(),
            machine: "machine-b".into(),
            boot: "boot-b".into(),
        };
        assert!(!store
            .observe_session_recovery_boot("stranded", &released, &other_host)
            .unwrap());
        let restarted = HostBoot {
            boot: "boot-b".into(),
            ..boot
        };
        assert!(store
            .observe_session_recovery_boot("stranded", &released, &restarted)
            .unwrap());
        let history = store.session_history("stranded", 0, 100).unwrap();
        assert!(history.iter().any(|event| event.payload == witness));
        assert!(history
            .iter()
            .any(|event| event.payload["type"] == "recovered_after_restart"
                && event.payload["previous"] == witness));
        let replacement = store
            .claim_session_driver("stranded", Some(&released), &process, true)
            .unwrap();
        assert_eq!(
            replacement.provider_generation,
            released.provider_generation + 1
        );
        assert!(store
            .record_session_provider_process("stranded", &driver, 123, 1)
            .is_err());
        assert!(store
            .observe_session_recovery_boot("stranded", &released, &restarted)
            .is_err());
        assert_eq!(
            store.session_thread("stranded").unwrap().as_deref(),
            Some("saved-native-thread")
        );
        assert_eq!(
            store.process(&process).unwrap().unwrap().outcome.as_deref(),
            Some("failed")
        );
        assert!(store
            .session_history("stranded", 0, 100)
            .unwrap()
            .iter()
            .all(|event| { event.kind != crate::session::SessionEventKind::Completed }));
        // A new driver cannot reuse the old witness, even if it fails again.
        let released = store
            .release_session_driver("stranded", &replacement)
            .unwrap();
        assert!(!store
            .observe_session_recovery_boot("stranded", &released, &restarted)
            .unwrap());
    }

    #[test]
    fn retained_process_identity_takes_precedence_over_restart_recovery() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        store.test_session("engine", &crate::session_record::new_artifact_key());
        let sql = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let process = ProcessLfid::new();
        sql.execute(
            "INSERT INTO processes(lfid,trace_id,started_at) VALUES(?1,'00000000-0000-0000-0000-000000000001',1)",
            [process.as_str()],
        )
        .unwrap();
        let driver = store
            .claim_session_driver("engine", None, &process, true)
            .unwrap();
        store
            .record_session_provider_process(
                "engine",
                &driver,
                std::process::id(),
                crate::journal::process_started_at(std::process::id())
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
        let released = store.release_session_driver("engine", &driver).unwrap();
        let boot = HostBoot {
            host: "fixture-host".into(),
            machine: "machine".into(),
            boot: "boot".into(),
        };
        assert!(!store
            .observe_session_recovery_boot("engine", &released, &boot)
            .unwrap());
        assert!(!crate::session_record::conversation_engine_exited(&store, "engine").unwrap());
        assert!(store
            .session_history("engine", 0, 100)
            .unwrap()
            .iter()
            .all(|event| { event.payload["type"] != "recovery_boot" }));
    }
}
