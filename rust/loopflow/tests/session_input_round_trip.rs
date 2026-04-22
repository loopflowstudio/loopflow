mod support;

use std::sync::Arc;
use std::time::Duration;

use loopflow::lfd::id::LfdId;
use loopflow::lfd::sessions::types::{
    CreateSessionParams, Session, SessionConfig, SessionEvent, SessionStatus,
};
use loopflow::lfd::sessions::{SessionManager, SessionManagerError};
use loopflow::lfd::store::{open_store, StorageConfig};
use tempfile::tempdir;

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

async fn wait_for_status(
    manager: &SessionManager,
    session_id: &LfdId,
    expected: SessionStatus,
) -> Session {
    for _ in 0..100 {
        let session = manager
            .get_session(session_id)
            .await
            .expect("session should exist");
        if session.status == expected {
            return session;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("session never reached expected status");
}

async fn wait_for_event(
    manager: &SessionManager,
    session_id: &LfdId,
    matches_event: impl Fn(&SessionEvent) -> bool,
) {
    for _ in 0..100 {
        let events = manager
            .list_events(session_id, None)
            .await
            .expect("list events");
        if events.iter().any(|event| matches_event(&event.event)) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("expected session event was not observed");
}

fn last_seq(events: &[loopflow::lfd::sessions::types::PersistedSessionEvent]) -> i64 {
    events.last().map(|event| event.seq).unwrap_or(-1)
}

#[tokio::test]
async fn codex_input_steers_running_turn_starts_idle_turn_and_replays_events() {
    let _env = support::EnvGuard::new(&[("codex", FAKE_CODEX)]);
    let tmp = tempdir().expect("tempdir");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".lf/steps")).expect("create repo steps");
    std::fs::write(repo.join(".lf/steps/design.md"), "Design the thing.").expect("write step");

    let store = Arc::new(
        open_store(&StorageConfig::sqlite(tmp.path().join("lfd.db")))
            .await
            .expect("open store"),
    );
    let manager = SessionManager::new(store.clone());

    let created = manager
        .create_session(CreateSessionParams {
            harness: "codex".to_string(),
            wave_run_id: None,
            config: SessionConfig {
                step: "design".to_string(),
                repo_root: repo.to_string_lossy().to_string(),
                ..Default::default()
            },
        })
        .await
        .expect("create codex session");
    assert!(loopflow::lfd::sessions::session_input_supported(
        &created.harness
    ));
    let _ = wait_for_status(&manager, &created.id, SessionStatus::Active).await;

    manager
        .send_input(&created.id, "first request")
        .await
        .expect("start first turn");
    wait_for_event(
        &manager,
        &created.id,
        |event| matches!(event, SessionEvent::TurnStarted { turn_id } if turn_id == "turn_1"),
    )
    .await;

    manager
        .send_input(&created.id, "also check the tests")
        .await
        .expect("steer running turn");
    wait_for_event(&manager, &created.id, |event| {
        matches!(event, SessionEvent::TextDelta { turn_id, content } if turn_id == "turn_1" && content.contains("steered:also check the tests"))
    })
    .await;
    wait_for_event(
        &manager,
        &created.id,
        |event| matches!(event, SessionEvent::TurnCompleted { turn_id, .. } if turn_id == "turn_1"),
    )
    .await;

    let before_second = manager
        .list_events(&created.id, None)
        .await
        .expect("list events before second turn");
    let after_seq = last_seq(&before_second);

    manager
        .send_input(&created.id, "second request")
        .await
        .expect("start second turn");
    wait_for_event(
        &manager,
        &created.id,
        |event| matches!(event, SessionEvent::TurnStarted { turn_id } if turn_id == "turn_2"),
    )
    .await;
    wait_for_event(
        &manager,
        &created.id,
        |event| matches!(event, SessionEvent::TurnCompleted { turn_id, .. } if turn_id == "turn_2"),
    )
    .await;

    let all_events = manager
        .list_events(&created.id, None)
        .await
        .expect("list all events");
    let replayed = manager
        .list_events(&created.id, Some(after_seq))
        .await
        .expect("replay missed events");
    let expected: Vec<_> = all_events
        .iter()
        .filter(|event| event.seq > after_seq)
        .map(|event| event.seq)
        .collect();
    let actual: Vec<_> = replayed.iter().map(|event| event.seq).collect();
    assert_eq!(actual, expected);
    assert!(replayed.iter().any(|event| {
        matches!(&event.event, SessionEvent::TurnStarted { turn_id } if turn_id == "turn_2")
    }));
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
        config: SessionConfig::default(),
        created_at: time::OffsetDateTime::now_utc(),
        ended_at: None,
    };
    store.create_session(&session).await.expect("seed session");

    let err = manager
        .send_input(&session_id, "hello")
        .await
        .expect_err("claude input should be rejected");
    assert!(
        matches!(err, SessionManagerError::InputNotSupported(ref harness) if harness == "claude")
    );
    assert!(!loopflow::lfd::sessions::session_input_supported("claude"));
}
