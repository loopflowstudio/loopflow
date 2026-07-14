//! Live smoke for the full two-process wave topology: a real `lf wave`
//! listener that spawns an internal resident, which runs
//! the real codex app-server.
//!
//! Ignored by default: it needs the codex CLI on PATH, ChatGPT auth, and
//! network, and spends (a trivial number of) tokens. Run manually:
//!
//! ```sh
//! cargo test -p loopflow --test wave_live_smoke -- --ignored
//! ```

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed");
}

async fn get_json(url: &str) -> Option<serde_json::Value> {
    reqwest::get(url).await.ok()?.json().await.ok()
}

/// Poll `url` every 500ms until `pred` accepts the body or `timeout` passes.
async fn poll_json(
    what: &str,
    url: &str,
    timeout: Duration,
    pred: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(body) = get_json(url).await {
            if pred(&body) {
                return body;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for: {what}"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::test]
#[ignore = "requires codex-cli on PATH, ChatGPT auth, and network; spends tokens"]
async fn wave_two_process_live_smoke() {
    // A throwaway repo with a demo wave.
    let tmp = tempfile::tempdir().expect("tempdir");
    let repo = tmp.path().join("demo-repo");
    std::fs::create_dir_all(repo.join("wave/demo")).expect("wave dir");
    git(&repo, &["init", "-q", "-b", "main", "."]);
    git(&repo, &["config", "user.name", "wave-smoke"]);
    git(&repo, &["config", "user.email", "smoke@loopflow.studio"]);
    std::fs::write(
        repo.join("wave/demo/GOAL.md"),
        "Answer questions briefly. Do not dispatch workers.\n",
    )
    .expect("goal");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "seed"]);

    // The listener; it spawns the resident itself (keeper spawns tenant).
    let mut listener = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["wave", "demo"])
        .current_dir(&repo)
        // A private registry so the smoke never touches the machine's ~/.lf.
        .env("LF_DB_PATH", tmp.path().join("loopflow.db"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn lf wave");

    let endpoint_file = repo.join("wave/demo/.wave-endpoint");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let addr = loop {
        if let Some(addr) = std::fs::read_to_string(&endpoint_file)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            break addr;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "endpoint never published"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    let base = format!("http://{addr}");

    // The resident attaches: the loop reports.
    poll_json(
        "resident attached (loop reported)",
        &format!("{base}/health"),
        Duration::from_secs(120),
        |body| body["status"] == "serving" && body["loop_state"] == "idle",
    )
    .await;

    // One real turn through the whole topology: door → journal → inbox SSE →
    // resident → codex → deltas → journal → conversation.
    let client = reqwest::Client::new();
    client
        .post(format!("{base}/messages"))
        .json(&serde_json::json!({ "op": "message", "text": "Reply with exactly OK" }))
        .send()
        .await
        .expect("post message")
        .error_for_status()
        .expect("message accepted");
    poll_json(
        "assistant turn finalized",
        &format!("{base}/conversation"),
        Duration::from_secs(180),
        |body| {
            body["turns"]
                .as_array()
                .and_then(|turns| turns.last())
                .is_some_and(|turn| {
                    turn["role"] == "assistant"
                        && turn["status"] != "running"
                        && turn["status"] != "pending"
                })
        },
    )
    .await;

    // Teardown: SIGTERM the listener; its hooks TERM the resident (which
    // stops codex) and remove the discovery files.
    let pid = listener.id();
    Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .expect("kill runs");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(Some(_)) = listener.try_wait() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "listener ignored SIGTERM"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(
        !endpoint_file.exists(),
        "endpoint pointer removed on shutdown"
    );
}
