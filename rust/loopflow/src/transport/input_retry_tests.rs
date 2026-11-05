//! Exercise a failed or locked store through the real descriptor loop in an
//! isolated process: input the store cannot retain still reaches the provider,
//! once, a locked store holds it no longer than the hook budget, and the
//! restored store learns of the delivered draft without another keystroke.

use std::cell::Cell;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use super::wire::{frame, Incoming, Request, Tag};
use crate::store::sqlite::sessions::OwnerRetention;

thread_local! {
    static POLLS: Cell<usize> = const { Cell::new(0) };
}

pub(super) fn poll() {
    POLLS.set(POLLS.get() + 1);
}

const FIXTURE_ENV: &str = "LOOPFLOW_INPUT_RETRY_FIXTURE";

#[test]
fn failed_input_store_delivers_the_queued_bytes_unrecorded() {
    run_fixture("failed");
}

#[test]
fn locked_input_store_delivers_the_queued_bytes_within_the_hook_budget() {
    run_fixture("locked");
}

fn run_fixture(store: &str) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "transport::input_retry_tests::input_store_fixture",
            "--nocapture",
        ])
        .env(FIXTURE_ENV, store)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Delivery may take the one-second hook budget while the store stays locked.
const DELIVERY: Duration = Duration::from_secs(2);

/// A restored store hears the delivered draft at the owner's next replay, one
/// lifetime poll away.
const CATCH_UP: Duration = super::lifetime::POLL.saturating_add(Duration::from_secs(3));

/// Break the store under the owner the way `kind` names, until restored.
fn break_store(kind: &str, db: &Path) -> rusqlite::Connection {
    let sql = rusqlite::Connection::open(db).unwrap();
    let sql_text = match kind {
        "failed" => {
            "CREATE TRIGGER reject_input BEFORE UPDATE OF native_input ON processes
             BEGIN SELECT RAISE(FAIL, 'fixture store unavailable'); END;"
        }
        // An install's migration: one exclusive transaction for as long as it runs.
        "locked" => "BEGIN EXCLUSIVE",
        other => panic!("unknown fixture store {other}"),
    };
    sql.execute_batch(sql_text).unwrap();
    sql
}

fn restore_store(sql: rusqlite::Connection) {
    let _ = sql.execute_batch("ROLLBACK");
    sql.execute_batch("DROP TRIGGER IF EXISTS reject_input")
        .unwrap();
}

#[test]
fn input_store_fixture() {
    let Some(kind) = std::env::var(FIXTURE_ENV).ok() else {
        return;
    };
    let guard = crate::journal::TestLedgerGuard::new();
    crate::journal::with_runtime(guard.home(), &["lf".into(), "fixture".into()], || {
        let owner = crate::journal::current_lf_process_id().unwrap();
        let broken = break_store(&kind, &guard.home().join("loopflow.db"));
        let endpoint = super::endpoint(&owner)?;
        let marker = guard.home().join("delivered");
        let release = guard.home().join("released");
        let (receipt, released, db) = (
            marker.clone(),
            release.clone(),
            guard.home().join("loopflow.db"),
        );
        let typed = owner.clone();
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
                    ssh: None,
                },
            };
            socket
                .write_all(&frame(Tag::Request, &serde_json::to_vec(&attach).unwrap()))
                .unwrap();
            socket
                .write_all(&frame(Tag::Input, b"retained draft\n"))
                .unwrap();
            let deadline = Instant::now() + DELIVERY;
            while !receipt.exists() {
                assert!(
                    Instant::now() < deadline,
                    "a {kind} store held the person's input past {DELIVERY:?}"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            restore_store(broken);
            // Nothing more is typed; the owner offers the delivered draft again
            // and the store, now answering, records it as retained input.
            let store = crate::store::sqlite::SqliteStore::open_existing_processes(
                &db,
                Duration::from_secs(1),
            )
            .unwrap();
            let deadline = Instant::now() + CATCH_UP;
            loop {
                if let Ok(Some(OwnerRetention::NativeInput)) = store.owner_retention(&typed) {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "the restored store never learned of the delivered draft"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            std::fs::write(released, "1").unwrap();
            let mut output = Vec::new();
            socket.read_to_end(&mut output).unwrap();
        });
        POLLS.set(0);
        let result = super::serve(
            &[
                "/bin/bash".into(),
                "-c".into(),
                "IFS= read -r -t 10 answer && printf '%s' \"$answer\" > \"$1\" \
                 && until [ -e \"$2\" ]; do sleep 0.05; done"
                    .into(),
                "fixture".into(),
                marker.to_string_lossy().into_owned(),
                release.to_string_lossy().into_owned(),
            ],
            true,
        );
        peer.join().unwrap();
        result?;
        assert_eq!(std::fs::read_to_string(marker)?, "retained draft");
        assert!(POLLS.get() < 100, "broken store spun on the writable PTY");
        Ok(())
    })
    .unwrap();
}
