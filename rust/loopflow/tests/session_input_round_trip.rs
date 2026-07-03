mod support;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use loopflow::lfd::auth::{AuthFailureThrottle, AuthProvider};
use loopflow::lfd::config::{ExecutorConfig, GitHubConfig, HttpSecurityConfig};
use loopflow::lfd::conversations::types::{
    Conversation, ConversationConfig, ConversationEvent, ConversationStatus,
    CreateConversationParams,
};
use loopflow::lfd::conversations::ConversationManager;
use loopflow::lfd::events::EventHub;
use loopflow::lfd::executor::WaveExecutor;
use loopflow::lfd::http::{router, HttpState};
use loopflow::lfd::id::LfdId;
use loopflow::lfd::output::OutputHub;
use loopflow::lfd::provider_auth::ProviderAuthService;
use loopflow::lfd::scheduler::Scheduler;
use loopflow::lfd::store::{open_store, StorageConfig};
use reqwest::Client;
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

const TEST_TOKEN: &str = "test-token";

const FAKE_CODEX: &str = r#"#!/usr/bin/env bash
set -euo pipefail

turn_index=0

send() {
    printf '{"jsonrpc":"2.0","method":"%s","params":%s}\n' "$1" "$2"
}

extract_content() {
    sed -n 's/.*"content":"\([^"]*\)".*/\1/p'
}

run_turn() {
    turn_index=$((turn_index + 1))
    turn_id="turn_${turn_index}"
    send "turn/started" "{\"turn\":{\"id\":\"${turn_id}\"}}"
    send "item/agentMessage/delta" "{\"turn\":{\"id\":\"${turn_id}\"},\"content\":\"started\"}"

    for _ in {1..3}; do
        if ! IFS= read -r -t 1 line; then
            continue
        fi
        case "$line" in
            *turn/steer*)
                content=$(printf '%s\n' "$line" | extract_content)
                send "item/agentMessage/delta" "{\"turn\":{\"id\":\"${turn_id}\"},\"content\":\"steered:${content}\"}"
                ;;
            *turn/interrupt*)
                send "turn/completed" "{\"turn\":{\"id\":\"${turn_id}\",\"status\":\"interrupted\"}}"
                return
                ;;
            *initialize*)
                send "initialized" "{}"
                ;;
        esac
    done

    send "turn/completed" "{\"turn\":{\"id\":\"${turn_id}\",\"status\":\"completed\"},\"usage\":{\"input_tokens\":1,\"output_tokens\":1},\"model\":\"fake-codex\"}"
}

send "initialized" "{}"

while IFS= read -r line; do
    case "$line" in
        *initialize*)
            send "initialized" "{}"
            ;;
        *thread/start*)
            ;;
        *turn/start*)
            run_turn
            ;;
    esac
done
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
    let sessions = ConversationManager::new(store.clone());
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
        conversations: sessions,
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

async fn create_session(
    sessions: &ConversationManager,
    harness: &str,
    repo: &Path,
) -> Conversation {
    sessions
        .create_conversation(CreateConversationParams {
            harness: harness.to_string(),
            run_id: None,
            config: ConversationConfig {
                step: "design".to_string(),
                repo_root: repo.to_string_lossy().to_string(),
                ..Default::default()
            },
        })
        .await
        .expect("create session")
}

async fn wait_for_session_status(
    conversations: &ConversationManager,
    conversation_id: &LfdId,
    expected: ConversationStatus,
) -> Value {
    let mut last_status = None;
    for _ in 0..1000 {
        let session = conversations
            .get_conversation(conversation_id)
            .await
            .expect("get session");
        last_status = Some(session.status);
        if session.status == expected {
            return json!({
                "id": session.id.to_string(),
                "status": session.status.as_str(),
                "input_supported": session.harness == "codex",
            });
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let events = conversations
        .list_events(conversation_id, None)
        .await
        .expect("list events");
    panic!(
        "session never reached expected status {expected:?}; last status: {last_status:?}; events: {events:?}"
    );
}

async fn send_input(client: &Client, base_url: &str, conversation_id: &str, text: &str) -> Value {
    let response = client
        .post(format!(
            "{base_url}/v0/conversations/{conversation_id}/input"
        ))
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
    conversation_id: &str,
    after_seq: Option<i64>,
    matches_event: impl Fn(&ConversationEvent) -> bool,
) -> Vec<(i64, ConversationEvent)> {
    let mut url = format!("{base_url}/v0/conversations/{conversation_id}/events");
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
            .unwrap_or_else(|_| {
                panic!("timed out waiting for SSE chunk; events={events:?}; buffer={buffer:?}")
            })
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

            if event_name != "conversation.event" {
                continue;
            }

            let seq = id.expect("conversation event id");
            let event: ConversationEvent =
                serde_json::from_str(&data).expect("conversation event json");
            let matched = matches_event(&event);
            events.push((seq, event));
            if matched {
                return events;
            }
        }
    }
}

fn last_seq(events: &[(i64, ConversationEvent)]) -> i64 {
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
    let sessions = state.conversations.clone();
    let (base_url, server) = start_server(state).await;
    let client = Client::new();

    let created = create_session(&sessions, "codex", &repo).await;
    assert_eq!(created.harness, "codex");
    let conversation_id = created.id.clone();
    let session_id_text = conversation_id.to_string();
    let _ = wait_for_session_status(&sessions, &conversation_id, ConversationStatus::Active).await;

    let first_response = send_input(&client, &base_url, &session_id_text, "first request").await;
    assert_eq!(first_response["input_supported"], true);
    let first_events = collect_sse_until(
        &client,
        &base_url,
        &session_id_text,
        None,
        |event| matches!(event, ConversationEvent::TurnStarted { turn_id } if turn_id == "turn_1"),
    )
    .await;

    let _ = send_input(&client, &base_url, &session_id_text, "also check the tests").await;
    let steered_events =
        collect_sse_until(&client, &base_url, &session_id_text, Some(last_seq(&first_events)), |event| {
        matches!(event, ConversationEvent::TextDelta { turn_id, content } if turn_id == "turn_1" && content.contains("steered:also check the tests"))
    })
    .await;
    let completed_first = collect_sse_until(
        &client,
        &base_url,
        &session_id_text,
        Some(last_seq(&steered_events)),
        |event| matches!(event, ConversationEvent::TurnCompleted { turn_id, .. } if turn_id == "turn_1"),
    )
    .await;
    let after_seq = last_seq(&completed_first);

    let _ = send_input(&client, &base_url, &session_id_text, "second request").await;
    let replayed = collect_sse_until(
        &client,
        &base_url,
        &session_id_text,
        Some(after_seq),
        |event| matches!(event, ConversationEvent::TurnCompleted { turn_id, .. } if turn_id == "turn_2"),
    )
    .await;
    assert!(replayed.iter().any(|(_, event)| {
        matches!(event, ConversationEvent::TurnStarted { turn_id } if turn_id == "turn_2")
    }));
    for (offset, (seq, _)) in replayed.iter().enumerate() {
        assert_eq!(*seq, after_seq + offset as i64 + 1);
    }

    let claude = create_session(&sessions, "claude", &repo).await;
    let claude_id = claude.id.clone();
    let claude_id_text = claude_id.to_string();
    let claude = wait_for_session_status(&sessions, &claude_id, ConversationStatus::Active).await;
    assert_eq!(claude["input_supported"], false);
    let response = client
        .post(format!(
            "{base_url}/v0/conversations/{claude_id_text}/input"
        ))
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
    let _env = support::EnvGuard::new(&[]);
    let tmp = tempdir().expect("tempdir");
    let store = Arc::new(
        open_store(&StorageConfig::sqlite(tmp.path().join("lfd.db")))
            .await
            .expect("open store"),
    );
    let manager = ConversationManager::new(store.clone());
    let conversation_id = LfdId::new();
    let session = Conversation {
        id: conversation_id.clone(),
        harness: "claude".to_string(),
        status: ConversationStatus::Active,
        run_id: None,
        provider_session_id: None,
        config: Default::default(),
        created_at: time::OffsetDateTime::now_utc(),
        ended_at: None,
    };
    store
        .create_conversation(&session)
        .await
        .expect("seed session");

    let err = manager
        .send_input(&conversation_id, "hello")
        .await
        .expect_err("claude input should be rejected");
    assert!(matches!(
        err,
        loopflow::lfd::conversations::ConversationManagerError::InputNotSupported(ref harness)
            if harness == "claude"
    ));
}
