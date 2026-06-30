mod support;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use loopflow::lfd::auth::{AuthFailureThrottle, AuthProvider};
use loopflow::lfd::config::{ExecutorConfig, GitHubConfig, HttpSecurityConfig};
use loopflow::lfd::events::EventHub;
use loopflow::lfd::executor::WaveExecutor;
use loopflow::lfd::http::{router, HttpState};
use loopflow::lfd::id::LfdId;
use loopflow::lfd::output::OutputHub;
use loopflow::lfd::provider_auth::ProviderAuthService;
use loopflow::lfd::scheduler::Scheduler;
use loopflow::lfd::sessions::types::{Session, SessionEvent, SessionStatus};
use loopflow::lfd::sessions::SessionManager;
use loopflow::lfd::store::{open_store, StorageConfig};
use reqwest::Client;
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

const TEST_TOKEN: &str = "test-token";

const FAKE_CODEX: &str = r#"#!/usr/bin/env python3
import json
import select
import sys
import time

turn_index = 0

def send(method, params=None):
    payload = {"jsonrpc": "2.0", "method": method, "params": params or {}}
    print(json.dumps(payload), flush=True)

def run_turn(content):
    global turn_index
    turn_index += 1
    turn_id = f"turn_{turn_index}"
    send("turn/started", {"turn": {"id": turn_id}})
    send("item/agentMessage/delta", {"turn": {"id": turn_id}, "content": f"started:{content}"})

    deadline = time.time() + 0.8
    while time.time() < deadline:
        timeout = max(0.0, deadline - time.time())
        ready, _, _ = select.select([sys.stdin], [], [], timeout)
        if not ready:
            break
        line = sys.stdin.readline()
        if not line:
            sys.exit(0)
        message = json.loads(line)
        method = message.get("method")
        params = message.get("params") or {}
        if method == "turn/steer":
            send(
                "item/agentMessage/delta",
                {"turn": {"id": turn_id}, "content": f"steered:{params.get('content', '')}"},
            )
        elif method == "turn/interrupt":
            send("turn/completed", {"turn": {"id": turn_id, "status": "interrupted"}})
            return
        elif method == "initialize":
            send("initialized")
        elif method == "thread/start":
            pass

    send(
        "turn/completed",
        {
            "turn": {"id": turn_id, "status": "completed"},
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "model": "fake-codex",
        },
    )

for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    params = message.get("params") or {}
    if method == "initialize":
        send("initialized")
    elif method == "thread/start":
        pass
    elif method == "turn/start":
        run_turn(params.get("content", ""))
    elif method == "turn/interrupt":
        pass
"#;

const FAKE_CLAUDE: &str = r#"#!/usr/bin/env python3
import sys

if "--version" in sys.argv:
    print("claude 0.0.0-test")
    sys.exit(0)

sys.exit(1)
"#;

async fn test_http_state(root: &Path) -> HttpState {
    let store: loopflow::lfd::store::SharedStore = Arc::new(
        open_store(&StorageConfig::sqlite(root.join("lfd.db")))
            .await
            .expect("open sqlite store"),
    );
    let scheduler = Arc::new(Scheduler::new(1));
    let output_hub = OutputHub::new(128, root.join("output"));
    let event_hub = EventHub::new(128);
    let sessions = SessionManager::new(store.clone());
    let executor = Arc::new(
        WaveExecutor::new(
            store.clone(),
            scheduler.clone(),
            output_hub.clone(),
            event_hub.clone(),
            ExecutorConfig::default(),
            GitHubConfig::default(),
        )
        .expect("build executor"),
    );

    HttpState {
        store: store.clone(),
        scheduler,
        executor,
        event_hub,
        output_hub,
        provider_auth: ProviderAuthService::new(store),
        auth: AuthProvider::Bearer {
            session_token: secrecy::SecretString::from(TEST_TOKEN.to_string()),
        },
        started_at: time::OffsetDateTime::now_utc(),
        github: GitHubConfig::default(),
        http_security: HttpSecurityConfig::default(),
        auth_failure_throttle: AuthFailureThrottle::new(),
        ci_failure_cache: Arc::new(Mutex::new(std::collections::HashSet::new())),
        sessions,
    }
}

async fn start_server(state: HttpState) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr: SocketAddr = listener.local_addr().expect("listener addr");
    let app = router(state);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve app");
    });
    (format!("http://{addr}"), server)
}

async fn create_session(client: &Client, base_url: &str, harness: &str, repo: &Path) -> Value {
    let response = client
        .post(format!("{base_url}/v0/sessions"))
        .bearer_auth(TEST_TOKEN)
        .json(&json!({
            "harness": harness,
            "step": "design",
            "repo_root": repo,
        }))
        .send()
        .await
        .expect("create session request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response.json().await.expect("create session json")
}

async fn get_session(client: &Client, base_url: &str, session_id: &str) -> Value {
    let response = client
        .get(format!("{base_url}/v0/sessions/{session_id}"))
        .bearer_auth(TEST_TOKEN)
        .send()
        .await
        .expect("get session request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response.json().await.expect("get session json")
}

async fn wait_for_http_status(
    client: &Client,
    base_url: &str,
    session_id: &str,
    expected: SessionStatus,
) -> Value {
    for _ in 0..100 {
        let session = get_session(client, base_url, session_id).await;
        if session["status"] == expected.as_str() {
            return session;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("session never reached expected status");
}

async fn send_input(client: &Client, base_url: &str, session_id: &str, text: &str) -> Value {
    let response = client
        .post(format!("{base_url}/v0/sessions/{session_id}/input"))
        .bearer_auth(TEST_TOKEN)
        .json(&json!({ "text": text }))
        .send()
        .await
        .expect("send input request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response.json().await.expect("send input json")
}

async fn collect_sse_until(
    client: &Client,
    base_url: &str,
    session_id: &str,
    after_seq: Option<i64>,
    matches_event: impl Fn(&SessionEvent) -> bool,
) -> Vec<(i64, SessionEvent)> {
    let mut url = format!("{base_url}/v0/sessions/{session_id}/events");
    if let Some(seq) = after_seq {
        url.push_str(&format!("?after_seq={seq}"));
    }
    let response = client
        .get(url)
        .bearer_auth(TEST_TOKEN)
        .send()
        .await
        .expect("sse request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let mut response = response;
    let mut buffer = String::new();
    let mut events = Vec::new();

    loop {
        let chunk = tokio::time::timeout(Duration::from_secs(5), response.chunk())
            .await
            .expect("timed out waiting for SSE chunk")
            .expect("read SSE chunk")
            .expect("SSE stream ended before expected event");
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(index) = buffer.find("\n\n") {
            let raw = buffer[..index].to_string();
            buffer.drain(..index + 2);

            let mut id = None;
            let mut event_name = "";
            let mut data = String::new();
            for line in raw.lines() {
                let line = line.trim_end_matches('\r');
                if let Some(value) = line.strip_prefix("id:") {
                    id = value.trim().parse::<i64>().ok();
                } else if let Some(value) = line.strip_prefix("event:") {
                    event_name = value.trim();
                } else if let Some(value) = line.strip_prefix("data:") {
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(value.trim_start());
                }
            }

            if event_name != "session.event" {
                continue;
            }

            let seq = id.expect("session event id");
            let event: SessionEvent = serde_json::from_str(&data).expect("session event json");
            let matched = matches_event(&event);
            events.push((seq, event));
            if matched {
                return events;
            }
        }
    }
}

fn last_seq(events: &[(i64, SessionEvent)]) -> i64 {
    events.last().map(|(seq, _)| *seq).unwrap_or(-1)
}

#[tokio::test]
async fn http_session_input_steers_running_turn_starts_idle_turn_and_replays_events() {
    let _env = support::EnvGuard::new(&[("codex", FAKE_CODEX), ("claude", FAKE_CLAUDE)]);
    let tmp = tempdir().expect("tempdir");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".lf/steps")).expect("create repo steps");
    std::fs::write(repo.join(".lf/steps/design.md"), "Design the thing.").expect("write step");

    let state = test_http_state(tmp.path()).await;
    let (base_url, server) = start_server(state).await;
    let client = Client::new();

    let created = create_session(&client, &base_url, "codex", &repo).await;
    assert_eq!(created["input_supported"], true);
    let session_id = created["id"].as_str().expect("session id");
    let _ = wait_for_http_status(&client, &base_url, session_id, SessionStatus::Active).await;

    let first_response = send_input(&client, &base_url, session_id, "first request").await;
    assert_eq!(first_response["input_supported"], true);
    let first_events = collect_sse_until(
        &client,
        &base_url,
        session_id,
        None,
        |event| matches!(event, SessionEvent::TurnStarted { turn_id } if turn_id == "turn_1"),
    )
    .await;

    let _ = send_input(&client, &base_url, session_id, "also check the tests").await;
    let steered_events =
        collect_sse_until(&client, &base_url, session_id, Some(last_seq(&first_events)), |event| {
        matches!(event, SessionEvent::TextDelta { turn_id, content } if turn_id == "turn_1" && content.contains("steered:also check the tests"))
    })
    .await;
    let completed_first = collect_sse_until(
        &client,
        &base_url,
        session_id,
        Some(last_seq(&steered_events)),
        |event| matches!(event, SessionEvent::TurnCompleted { turn_id, .. } if turn_id == "turn_1"),
    )
    .await;
    let after_seq = last_seq(&completed_first);

    let _ = send_input(&client, &base_url, session_id, "second request").await;
    let replayed = collect_sse_until(
        &client,
        &base_url,
        session_id,
        Some(after_seq),
        |event| matches!(event, SessionEvent::TurnCompleted { turn_id, .. } if turn_id == "turn_2"),
    )
    .await;
    assert!(replayed.iter().any(|(_, event)| {
        matches!(event, SessionEvent::TurnStarted { turn_id } if turn_id == "turn_2")
    }));
    for (offset, (seq, _)) in replayed.iter().enumerate() {
        assert_eq!(*seq, after_seq + offset as i64 + 1);
    }

    let claude = create_session(&client, &base_url, "claude", &repo).await;
    let claude_id = claude["id"].as_str().expect("claude session id");
    let claude = wait_for_http_status(&client, &base_url, claude_id, SessionStatus::Active).await;
    assert_eq!(claude["input_supported"], false);
    let response = client
        .post(format!("{base_url}/v0/sessions/{claude_id}/input"))
        .bearer_auth(TEST_TOKEN)
        .json(&json!({ "text": "hello" }))
        .send()
        .await
        .expect("claude input request");
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let error: Value = response.json().await.expect("claude input error json");
    assert!(error["error"]["message"]
        .as_str()
        .expect("error message")
        .contains("input not supported for this harness"));

    server.abort();
}

#[tokio::test]
async fn non_codex_session_input_is_rejected_before_runtime_lookup() {
    let tmp = tempdir().expect("tempdir");
    let store = Arc::new(
        open_store(&StorageConfig::sqlite(tmp.path().join("lfd.db")))
            .await
            .expect("open store"),
    );
    let manager = SessionManager::new(store.clone());
    let session_id = LfdId::new();
    let session = Session {
        id: session_id.clone(),
        harness: "claude".to_string(),
        status: SessionStatus::Active,
        wave_run_id: None,
        provider_session_id: None,
        config: Default::default(),
        created_at: time::OffsetDateTime::now_utc(),
        ended_at: None,
    };
    store.create_session(&session).await.expect("seed session");

    let err = manager
        .send_input(&session_id, "hello")
        .await
        .expect_err("claude input should be rejected");
    assert!(matches!(
        err,
        loopflow::lfd::sessions::SessionManagerError::InputNotSupported(ref harness)
            if harness == "claude"
    ));
}
