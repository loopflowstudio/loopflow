//! Exercise a failed store through the real descriptor loop in an isolated process.

use std::cell::Cell;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::{Duration, Instant};

use super::wire::{frame, Incoming, Request, Tag};

thread_local! {
    static POLLS: Cell<usize> = const { Cell::new(0) };
}

pub(super) fn poll() {
    POLLS.set(POLLS.get() + 1);
}

#[test]
fn failed_input_store_waits_then_delivers_the_same_queued_bytes() {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "transport::input_retry_tests::input_store_fixture",
            "--nocapture",
        ])
        .env("LOOPFLOW_INPUT_RETRY_FIXTURE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn input_store_fixture() {
    if std::env::var_os("LOOPFLOW_INPUT_RETRY_FIXTURE").is_none() {
        return;
    }
    let guard = crate::journal::TestLedgerGuard::new();
    crate::journal::with_runtime(guard.home(), &["lf".into(), "fixture".into()], || {
        let owner = crate::journal::current_lf_process_id().unwrap();
        let db = guard.home().join("loopflow.db");
        let sql = rusqlite::Connection::open(&db)?;
        sql.execute_batch(
            "CREATE TRIGGER reject_input BEFORE UPDATE OF native_input ON processes
             BEGIN SELECT RAISE(FAIL, 'fixture store unavailable'); END;",
        )?;
        let endpoint = super::endpoint(&owner)?;
        let marker = guard.home().join("delivered");
        let receipt = marker.clone();
        let peer = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut socket = loop {
                if let Ok(socket) = UnixStream::connect(&endpoint) {
                    break socket;
                }
                assert!(Instant::now() < deadline, "owner did not open its socket");
                std::thread::sleep(Duration::from_millis(10));
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(12)))
                .unwrap();
            let attach = Incoming {
                id: 1,
                request: Request::Attach {
                    control: true,
                    replace: true,
                    own: None,
                    rows: 24,
                    cols: 80,
                    name: None,
                    moved: None,
                },
            };
            socket
                .write_all(&frame(Tag::Request, &serde_json::to_vec(&attach).unwrap()))
                .unwrap();
            socket
                .write_all(&frame(Tag::Input, b"retained draft\n"))
                .unwrap();
            std::thread::sleep(Duration::from_secs(3));
            assert!(!receipt.exists(), "unrecorded input reached the provider");
            let sql = rusqlite::Connection::open(db).unwrap();
            sql.execute_batch("DROP TRIGGER reject_input").unwrap();
            let mut output = Vec::new();
            socket.read_to_end(&mut output).unwrap();
        });
        POLLS.set(0);
        let result = super::serve(
            &[
                "/bin/bash".into(),
                "-c".into(),
                "IFS= read -r -t 10 answer && printf '%s' \"$answer\" > \"$1\"".into(),
                "fixture".into(),
                marker.to_string_lossy().into_owned(),
            ],
            true,
        );
        peer.join().unwrap();
        result?;
        assert_eq!(std::fs::read_to_string(marker)?, "retained draft");
        assert!(POLLS.get() < 100, "failed store spun on the writable PTY");
        Ok(())
    })
    .unwrap();
}
