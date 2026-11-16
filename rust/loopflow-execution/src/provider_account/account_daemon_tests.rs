//! Disposable socket + restart executable; never uses a configured Codex home.
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::UnixListener;
use tokio::task::{JoinHandle, JoinSet};
use tokio_tungstenite::tungstenite::Message;

use super::{restart_stale_codex_daemon, CODEX_DAEMON_TURN_WAIT};
use crate::provider_auth::codex::{daemon_has_running_turn, daemon_login};

struct Daemon {
    server: JoinHandle<()>,
    path: Option<OsString>,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.server.abort();
        match &self.path {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
    }
}

impl Daemon {
    fn start(home: &Path, reply: impl Fn(&Value) -> Value + Send + Sync + 'static) -> Self {
        let bin = home.join("bin");
        fs::create_dir(&bin).unwrap();
        let executable = bin.join("codex");
        fs::write(
            &executable,
            "#!/bin/sh\nprintf restarted > \"$CODEX_HOME/restarted\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::var_os("PATH");
        // No installed provider can be selected, including on restart failures.
        std::env::set_var("PATH", bin);
        let control = home.join("app-server-control");
        fs::create_dir(&control).unwrap();
        let listener = UnixListener::bind(control.join("app-server-control.sock")).unwrap();
        let reply = Arc::new(reply);
        let server = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (stream, _) = accepted.unwrap();
                        let reply = reply.clone();
                        connections.spawn(async move {
                            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                            while let Some(Ok(Message::Text(text))) = socket.next().await {
                                let request: Value = serde_json::from_str(&text).unwrap();
                                if request["id"].is_null() { continue; }
                                let result = if request["method"] == "initialize" {
                                    json!({})
                                } else { reply(&request) };
                                let message = json!({"id": request["id"], "result": result});
                                if socket.send(Message::Text(message.to_string().into())).await.is_err() { break; }
                            }
                        });
                    }
                    Some(finished) = connections.join_next() => { finished.unwrap(); }
                }
            }
        });
        Self { server, path }
    }
}

fn run(check: impl AsyncFnOnce(&Path)) {
    let _lock = crate::journal::test_env_lock();
    let home = tempfile::Builder::new().tempdir_in("/tmp").unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tokio::time::timeout(Duration::from_secs(20), check(home.path()))
                .await
                .unwrap();
        });
}

fn reply(home: &Path, request: &Value, status: &str) -> Value {
    match request["method"].as_str().unwrap() {
        "account/read" => {
            assert_eq!(request["params"]["refreshToken"], false);
            let login = if home.join("restarted").exists() {
                "new@example.com"
            } else {
                "old@example.com"
            };
            json!({"account": {"email": login}})
        }
        "thread/loaded/list" => json!({"data": ["thread-1"], "nextCursor": null}),
        "thread/read" => json!({"thread": {"status": {"type": status}}}),
        method => panic!("unexpected method: {method}"),
    }
}

#[test]
fn absent_and_matching_daemons_are_left_alone() {
    run(async |home| {
        restart_stale_codex_daemon(home, "new@example.com")
            .await
            .unwrap();
        let _daemon = Daemon::start(home, |_| json!({"account": {"email": "NEW@example.com"}}));
        restart_stale_codex_daemon(home, "new@example.com")
            .await
            .unwrap();
        assert!(!home.join("restarted").exists());
    });
}

#[test]
fn unreadable_login_is_an_error_without_restart() {
    run(async |home| {
        let _daemon = Daemon::start(home, |_| Value::Null);
        assert!(restart_stale_codex_daemon(home, "new@example.com")
            .await
            .is_err());
        assert!(!home.join("restarted").exists());
    });
}

#[test]
fn missing_email_and_activity_are_errors_not_absence_or_idle() {
    run(async |home| {
        let _daemon = Daemon::start(home, |_| json!({}));
        assert!(daemon_login(home).await.is_err());
        assert!(daemon_has_running_turn(home).await.is_err());
    });
}

#[test]
fn activity_on_a_later_page_is_not_idle() {
    run(async |home| {
        let _daemon = Daemon::start(home, |request| match request["method"].as_str().unwrap() {
            "thread/loaded/list" if request["params"]["cursor"].is_null() => {
                json!({"data": ["idle"], "nextCursor": "page-2"})
            }
            "thread/loaded/list" => json!({"data": ["busy"], "nextCursor": null}),
            _ => {
                json!({"thread": {"status": {"type": if request["params"]["threadId"] == "busy" { "active" } else { "idle" }}}})
            }
        });
        assert!(daemon_has_running_turn(home).await.unwrap());
    });
}

#[test]
fn busy_and_unreadable_turns_finish_before_restart() {
    run(async |home| {
        let native = home.to_path_buf();
        let reads = std::sync::atomic::AtomicUsize::new(0);
        let _daemon = Daemon::start(home, move |request| {
            let status = if request["method"] == "thread/read" {
                assert!(!native.join("restarted").exists(), "restarted before idle");
                match reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                    0 => "active",
                    1 => return Value::Null, // A failed read must not end the wait.
                    _ => {
                        fs::write(native.join("turn-finished"), "done").unwrap();
                        "idle"
                    }
                }
            } else {
                "idle"
            };
            reply(&native, request, status)
        });
        restart_stale_codex_daemon(home, "new@example.com")
            .await
            .unwrap();
        assert!(home.join("restarted").exists());
        assert!(
            home.join("turn-finished").exists(),
            "restarted before the turn finished"
        );
        assert_eq!(
            daemon_login(home).await.unwrap().as_deref(),
            Some("new@example.com")
        );
    });
}

#[test]
fn idle_daemon_restarts_and_failed_restart_is_reported() {
    run(async |home| {
        let native = home.to_path_buf();
        let _daemon = Daemon::start(home, move |request| reply(&native, request, "idle"));
        fs::write(home.join("bin/codex"), "#!/bin/sh\nexit 1\n").unwrap();
        assert!(restart_stale_codex_daemon(home, "new@example.com")
            .await
            .unwrap_err()
            .to_string()
            .contains("restart failed"));
        assert!(!home.join("restarted").exists());
    });
}

#[test]
fn restart_must_adopt_the_selected_login() {
    run(async |home| {
        let native = home.to_path_buf();
        let _daemon = Daemon::start(home, move |request| {
            if request["method"] == "account/read" {
                json!({"account": {"email": "old@example.com"}})
            } else {
                reply(&native, request, "idle")
            }
        });
        assert!(restart_stale_codex_daemon(home, "new@example.com")
            .await
            .unwrap_err()
            .to_string()
            .contains("still reports old@example.com"));
        assert!(home.join("restarted").exists());
    });
}

#[test]
fn stalled_handshake_is_an_error_not_an_absent_daemon() {
    run(async |home| {
        let control = home.join("app-server-control");
        fs::create_dir(&control).unwrap();
        let _listener = UnixListener::bind(control.join("app-server-control.sock")).unwrap();
        assert!(daemon_login(home)
            .await
            .unwrap_err()
            .to_string()
            .contains("timed out"));
        assert!(daemon_has_running_turn(home)
            .await
            .unwrap_err()
            .to_string()
            .contains("timed out"));
    });
}

#[test]
#[ignore = "real five-minute grace period; run explicitly at gate"]
fn expired_grace_restarts_even_when_activity_becomes_unreadable() {
    let _lock = crate::journal::test_env_lock();
    let home = tempfile::Builder::new().tempdir_in("/tmp").unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let native = home.path().to_path_buf();
            let reads = std::sync::atomic::AtomicUsize::new(0);
            let _daemon = Daemon::start(home.path(), move |request| {
                if request["method"] == "thread/read"
                    && reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0
                {
                    return Value::Null;
                }
                reply(&native, request, "active")
            });
            let started = Instant::now();
            tokio::time::timeout(
                CODEX_DAEMON_TURN_WAIT + Duration::from_secs(15),
                restart_stale_codex_daemon(home.path(), "new@example.com"),
            )
            .await
            .unwrap()
            .unwrap();
            assert!(started.elapsed() >= CODEX_DAEMON_TURN_WAIT);
            assert!(home.path().join("restarted").exists());
            assert_eq!(
                daemon_login(home.path()).await.unwrap().as_deref(),
                Some("new@example.com")
            );
        });
}
