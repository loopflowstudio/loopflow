//! Integration test for `lfd serve`: boots the real binary and proves the
//! durable delivery inbox — signed Linear webhook ingestion, idempotent dedup
//! across retries, and durable dedup across a daemon restart.

use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use axum::http::StatusCode;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use loopflow::store::StorageConfig;

type HmacSha256 = Hmac<Sha256>;

const SECRET: &str = "whsec_test";
const VIEWER_ID: &str = "viewer-test";

/// Kills the child on drop so a failing assertion cannot leak the daemon.
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Bind an ephemeral port and release it so the `lfd` subprocess can rebind.
fn reserve_port() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve port");
    listener.local_addr().expect("addr")
}

/// Poll `url` until `pred` accepts the JSON body or `timeout` passes.
async fn poll_json_until(
    what: &str,
    url: &str,
    timeout: Duration,
    pred: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Ok(resp) = reqwest::get(url).await {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if pred(&body) {
                    return body;
                }
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for: {what}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn sign(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).unwrap();
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Build a signed Linear Issue-update webhook with a fresh `webhookTimestamp`.
/// The `issue_id` and `updatedAt` revision are fixed so the delivery id
/// (`linear:issue:{issue_id}:{revision}`) is stable across sends — the inbox
/// dedup key must not change between retries or across a restart.
fn signed_issue_webhook(secret: &[u8]) -> (Vec<u8>, String) {
    let body = serde_json::json!({
        "action": "update",
        "type": "Issue",
        "data": {
            "id": "W2-224-test",
            "title": "Test issue",
            "description": "Test description",
            "updatedAt": "2026-07-16T01:00:00.000Z"
        },
        "updatedFrom": { "title": "Old title" },
        "actor": { "id": "user-human" },
        "webhookTimestamp": now_ms()
    });
    let body_bytes = serde_json::to_vec(&body).unwrap();
    let sig = sign(secret, &body_bytes);
    (body_bytes, sig)
}

fn spawn_lfd(repo: &std::path::Path, db_path: &std::path::Path, addr: SocketAddr) -> ChildGuard {
    let child = Command::new(env!("CARGO_BIN_EXE_lfd"))
        .arg("serve")
        .arg("--repo")
        .arg(repo)
        .arg("--addr")
        .arg(addr.to_string())
        .env("LF_DB_PATH", db_path)
        .env("LF_HOME", repo.join(".lf-test"))
        .env("LF_CONTROL_HOME", repo.join(".lf-test"))
        .env_remove("LF_CONTROL_DB_PATH")
        .env("LF_LINEAR_WEBHOOK_SECRET", SECRET)
        .env("LF_LINEAR_VIEWER_ID", VIEWER_ID)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn lfd");
    ChildGuard(child)
}

#[test]
fn lfd_reports_the_control_plane_build_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_lfd"))
        .arg("--version")
        .output()
        .expect("run lfd --version");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("lfd {}\n", loopflow::build_info::BUILD_VERSION)
    );
}

#[tokio::test]
async fn lfd_dedups_signed_deliveries_across_restart() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let db_path = repo.path().join("registry.db");

    // Create the store with migrations so `lfd serve` finds an existing db.
    let expected_home_id = {
        let store = loopflow::store::open_ephemeral_store(&StorageConfig::sqlite(db_path.clone()))
            .await
            .expect("create store");
        store.local_home().await.expect("read local Home").id
    };

    let lfd_addr = reserve_port();
    let _lfd = spawn_lfd(repo.path(), &db_path, lfd_addr);
    let base = format!("http://{lfd_addr}");

    // /health
    let health = poll_json_until(
        "lfd /health",
        &format!("{base}/health"),
        Duration::from_secs(15),
        |b| b["status"] == "ok",
    )
    .await;
    assert_eq!(health["status"], "ok");
    assert_eq!(health["home_id"], expected_home_id.as_str());
    let endpoint_path = repo
        .path()
        .join(".lf-test/lfd")
        .join(format!("{}.endpoint", expected_home_id.as_str()));
    let endpoint: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&endpoint_path).expect("read lfd endpoint"))
            .expect("parse lfd endpoint");
    assert_eq!(endpoint["endpoint"], lfd_addr.to_string());
    assert!(endpoint["token"]
        .as_str()
        .is_some_and(|token| !token.is_empty()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            std::fs::metadata(&endpoint_path)
                .expect("read lfd endpoint metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    // /status — fresh store, zero deliveries, zero wave endpoints.
    let status = poll_json_until(
        "lfd /status",
        &format!("{base}/status"),
        Duration::from_secs(15),
        |b| b["deliveries"] == 0,
    )
    .await;
    assert_eq!(status["waves"], 0);
    assert_eq!(status["deliveries"], 0);

    // Send a valid signed webhook. No Task matches the issue id, so the
    // delivery is classified `no_target` — but it is still persisted and 200'd.
    let (body, sig) = signed_issue_webhook(SECRET.as_bytes());
    let resp = reqwest::Client::new()
        .post(format!("{base}/linear/webhook"))
        .header("linear-signature", &sig)
        .body(body)
        .send()
        .await
        .expect("post webhook");
    assert_eq!(resp.status(), StatusCode::OK);

    // /status — one delivery recorded.
    let status = poll_json_until(
        "lfd /status after delivery",
        &format!("{base}/status"),
        Duration::from_secs(5),
        |b| b["deliveries"] == 1,
    )
    .await;
    assert_eq!(status["deliveries"], 1);

    // Send the same delivery (fresh timestamp, same issue_id + revision) — the
    // delivery id is identical, so the inbox drops it as a true duplicate.
    let (body2, sig2) = signed_issue_webhook(SECRET.as_bytes());
    let resp = reqwest::Client::new()
        .post(format!("{base}/linear/webhook"))
        .header("linear-signature", &sig2)
        .body(body2)
        .send()
        .await
        .expect("post duplicate");
    assert_eq!(resp.status(), StatusCode::OK);

    // Still one delivery — the duplicate was not re-processed.
    let status = reqwest::get(format!("{base}/status"))
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(status["deliveries"], 1);

    // Kill lfd and restart with the same store — the inbox is durable in
    // SQLite, so the same delivery id must still dedup across the restart.
    drop(_lfd);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _lfd2 = spawn_lfd(repo.path(), &db_path, lfd_addr);

    let _ = poll_json_until(
        "lfd restarted /health",
        &format!("{base}/health"),
        Duration::from_secs(15),
        |b| b["status"] == "ok",
    )
    .await;

    // Send the same delivery again — durable dedup across restart.
    let (body3, sig3) = signed_issue_webhook(SECRET.as_bytes());
    let resp = reqwest::Client::new()
        .post(format!("{base}/linear/webhook"))
        .header("linear-signature", &sig3)
        .body(body3)
        .send()
        .await
        .expect("post after restart");
    assert_eq!(resp.status(), StatusCode::OK);

    // Still one delivery — the durable inbox persisted across the restart.
    let status = reqwest::get(format!("{base}/status"))
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(status["deliveries"], 1);

    // Unsigned webhook → 401.
    let resp = reqwest::Client::new()
        .post(format!("{base}/linear/webhook"))
        .body("test")
        .send()
        .await
        .expect("post unsigned");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
