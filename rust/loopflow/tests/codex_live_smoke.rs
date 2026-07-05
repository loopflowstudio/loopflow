//! Live smoke test for the codex app-server driver.
//!
//! Ignored by default: it spawns the real `codex` binary, needs ChatGPT auth
//! and network, and spends (a trivial number of) tokens. Run manually:
//!
//! ```sh
//! cargo test -p loopflow --test codex_live_smoke -- --ignored
//! ```

use std::time::Duration;

use loopflow::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use loopflow::engine::agent::AgentConfig;
use loopflow::harness::codex::CodexHarness;
use loopflow::harness::{ApprovalPolicy, Harness};
use tokio::sync::mpsc;

#[tokio::test]
#[ignore = "requires codex-cli on PATH, ChatGPT auth, and network; spends tokens"]
async fn codex_live_one_turn_smoke() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut harness = CodexHarness::new(tx, ApprovalPolicy::AutoApprove);

    let config = AgentConfig {
        agent: Some("codex".to_string()),
        cwd: Some(dir.path().to_path_buf()),
        ..AgentConfig::default()
    };
    harness.start(&config).await.expect("codex start");
    assert!(
        harness.provider_session_id().is_some(),
        "thread id should be captured during start"
    );

    harness
        .send_input("Reply with exactly OK")
        .await
        .expect("send input");

    let mut completed_message: Option<String> = None;
    let mut turn_status: Option<Lifecycle> = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    while turn_status.is_none() {
        let event = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .expect("timed out waiting for turn events")
            .expect("event channel closed before turn completed");
        match event {
            ConversationEvent::ItemCompleted {
                item: ConversationItem::Message { text, .. },
                ..
            } => completed_message = Some(text),
            ConversationEvent::TurnCompleted { status, .. } => turn_status = Some(status),
            ConversationEvent::Error { code, message } => {
                panic!("harness error during turn: {code}: {message}")
            }
            _ => {}
        }
    }

    assert_eq!(turn_status, Some(Lifecycle::Completed));
    let text = completed_message.expect("agent message should complete before the turn ends");
    assert!(
        text.contains("OK"),
        "expected agent reply containing OK, got: {text:?}"
    );

    harness.stop().await.expect("codex stop");
}
