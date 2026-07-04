//! End-to-end round-trip for the per-wave chat server hosted by `lf wave`.
//!
//! Replaces the removed daemon-era `session_input_round_trip.rs`: the central
//! `/v0/conversations` HTTP surface no longer exists. This exercises the new
//! contract Concerto consumes — discovery file, `GET /health`, `GET /chat`,
//! `POST /chat`, and the `GET /chat/stream` SSE — plus the codex stream → turn
//! ingestion that feeds them.

use std::time::Duration;

use loopflow::engine::stream::StreamParser;
use loopflow::lfd::conversations::server::{self, ChatState};
use reqwest::Client;
use serde_json::Value;
use tempfile::tempdir;

async fn start(wave_dir: &std::path::Path) -> (String, std::sync::Arc<ChatState>) {
    let state = ChatState::new("demo".to_string(), wave_dir.to_path_buf());
    let listener = server::bind(wave_dir).await.expect("bind chat server");
    let addr = listener.local_addr().expect("addr");
    let router = server::router(state.clone());
    tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serve");
    });
    (format!("http://{addr}"), state)
}

#[tokio::test]
async fn chat_server_serves_turns_mailbox_and_stream() {
    let dir = tempdir().expect("tempdir");
    let wave_dir = dir.path();
    let (base, state) = start(wave_dir).await;
    let client = Client::new();

    // Discovery file points at the live server.
    let endpoint = std::fs::read_to_string(wave_dir.join(".chat-endpoint")).expect("endpoint file");
    assert!(endpoint.trim().starts_with("127.0.0.1:"));

    // Health is live and reports the wave.
    let health: Value = client
        .get(format!("{base}/health"))
        .send()
        .await
        .expect("health")
        .json()
        .await
        .expect("health json");
    assert_eq!(health["status"], "ok");
    assert_eq!(health["wave"], "demo");

    // A human message lands in the mailbox and shows as a user turn.
    let posted: Value = client
        .post(format!("{base}/chat"))
        .json(&serde_json::json!({ "text": "check the tests" }))
        .send()
        .await
        .expect("post chat")
        .json()
        .await
        .expect("post json");
    assert_eq!(posted["role"], "user");
    assert_eq!(posted["text"], "check the tests");
    let mailbox = std::fs::read_to_string(wave_dir.join("MAILBOX.md")).expect("mailbox");
    assert!(mailbox.contains("check the tests"));

    // Feed a codex `exec --json` turn; it becomes an assistant turn.
    let mut parser = StreamParser::new();
    state.begin_pass();
    state.ingest_line(
        &mut parser,
        r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"Ran the tests, all green."}}"#,
    );
    state.ingest_line(
        &mut parser,
        r#"{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":4}}"#,
    );

    let chat: Value = client
        .get(format!("{base}/chat"))
        .send()
        .await
        .expect("get chat")
        .json()
        .await
        .expect("chat json");
    let turns = chat["turns"].as_array().expect("turns array");
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0]["role"], "user");
    assert_eq!(turns[1]["role"], "assistant");
    assert_eq!(turns[1]["status"], "completed");
    assert_eq!(turns[1]["text"], "Ran the tests, all green.");

    // The SSE stream replays the current turns.
    let mut response = client
        .get(format!("{base}/chat/stream"))
        .send()
        .await
        .expect("sse request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let mut buffer = String::new();
    let mut saw_assistant = false;
    for _ in 0..20 {
        let chunk = tokio::time::timeout(Duration::from_secs(5), response.chunk())
            .await
            .expect("sse chunk timeout")
            .expect("sse chunk")
            .expect("sse stream ended");
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        if buffer.contains("Ran the tests, all green.") {
            saw_assistant = true;
            break;
        }
    }
    assert!(saw_assistant, "SSE stream should replay the assistant turn");
}
