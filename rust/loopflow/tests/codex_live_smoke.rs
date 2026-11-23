//! Live smoke test for the codex app-server client.
//!
//! Ignored by default: it spawns the real `codex` binary, needs ChatGPT auth
//! and network, and spends (a trivial number of) tokens. Run manually:
//!
//! ```sh
//! cargo test -p loopflow --test codex_live_smoke -- --ignored
//! ```

use loopflow_execution::process::attachment;
use std::time::Duration;

use loopflow_execution::agent::AgentConfig;
use loopflow_execution::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use loopflow_execution::harness::codex::Driver as CodexDriver;
use loopflow_execution::harness::{ApprovalPolicy, Driver};
use loopflow_execution::session::{LfSession, TitleSource};
use loopflow_execution::store::{open_ephemeral_store, StorageConfig};
use loopflow_model::id::LfProcessId;
use tokio::sync::mpsc;

#[tokio::test]
#[ignore = "requires codex-cli on PATH, ChatGPT auth, and network; spends tokens"]
async fn codex_live_one_turn_smoke() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Every launch is recorded: admit one Session in a private store.
    std::env::set_var("LF_HOME", dir.path());
    let database = dir.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .expect("store");
    let store = loopflow_execution::store::open_sqlite(&database).expect("store");
    loopflow_execution::store::sessions::create_session(
        &store,
        LfSession {
            captured: None,
            task_id: None,
            wave_id: None,
            graph_lf_process_id: None,
            work_source: None,
            bound_at: None,
            id: "smoke".into(),
            artifact_key: Some(uuid::Uuid::new_v4().simple().to_string()),
            caller_artifact_key: None,
            input_published: false,
            cwd: dir.path().into(),
            skill: None,
            provider: Some("codex".into()),
            model: None,
            node: None,
            iterations: None,
            interactive: false,
            repo: None,
            title: "Codex live smoke".into(),
            title_source: TitleSource::Generated,
            request: None,
            completed_at: None,
            created_at: 1,
        },
        None,
    )
    .expect("session");
    let parent = LfProcessId::new();
    rusqlite::Connection::open(&database)
        .expect("store")
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
            [&parent],
        )
        .expect("record attaching lf Process");
    let attachment =
        attachment::claim_session_attachment(&store, "smoke", None, &parent).expect("attachment");
    let config = AgentConfig {
        cwd: Some(dir.path().to_path_buf()),
        ..AgentConfig::new(loopflow_execution::config::Agent::parse("codex").unwrap())
    };
    let mut harness = CodexDriver::new(
        &loopflow_execution::harness::launch_for_test(
            loopflow_harness::HarnessKind::Codex,
            &config,
            &loopflow_execution::harness::Recording {
                store,
                session: "smoke".into(),
                attachment,
            },
        ),
        tx,
        ApprovalPolicy::AutoApprove,
    );
    harness.start().await.expect("codex start");
    assert!(
        harness.agent_session().is_some(),
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
            ConversationEvent::Error { code, message, .. } => {
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
