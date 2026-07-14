use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tokio::sync::mpsc;

use super::claude_mapping::{self, ReaderState};
use super::codex::{process_notification, process_rpc_error, NotificationState};
use super::opencode_mapping;
use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};

fn read_trace_lines(file_name: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/harness/testdata")
        .join(file_name);
    fs::read_to_string(path)
        .expect("trace file should exist")
        .lines()
        .map(ToString::to_string)
        .collect()
}

fn drain_events(
    rx: &mut mpsc::UnboundedReceiver<ConversationEvent>,
    out: &mut Vec<ConversationEvent>,
) {
    while let Ok(event) = rx.try_recv() {
        out.push(event);
    }
}

fn replay_claude_trace(file_name: &str) -> (Vec<ConversationEvent>, Option<String>) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut events = Vec::new();
    let mut state = ReaderState::default();
    let mut saw_turn_completed = false;

    for line in read_trace_lines(file_name) {
        if line.trim().is_empty() {
            continue;
        }
        if claude_mapping::process_line(&line, "turn_trace", &tx, &mut state) {
            saw_turn_completed = true;
        }
        drain_events(&mut rx, &mut events);
        if saw_turn_completed {
            break;
        }
    }

    let session_id = state.take_provider_session_id();

    if !saw_turn_completed {
        for item in state.drain_open_items(Lifecycle::Failed) {
            events.push(ConversationEvent::ItemCompleted {
                turn_id: "turn_trace".to_string(),
                item,
            });
        }
        events.push(ConversationEvent::TurnCompleted {
            turn_id: "turn_trace".to_string(),
            status: Lifecycle::Failed,
        });
    }

    (events, session_id)
}

fn replay_codex_trace(file_name: &str) -> Vec<ConversationEvent> {
    replay_codex_lines(read_trace_lines(file_name))
}

/// Replay codex app-server lines through the production notification
/// dispatch (`codex::process_notification`), so traces pin real behavior.
fn replay_codex_lines(lines: Vec<String>) -> Vec<ConversationEvent> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut state = NotificationState::new(
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(None)),
        Arc::new(Mutex::new(None)),
        None,
    );
    let mut events = Vec::new();

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(&line).expect("trace line should be valid json");
        let method = value
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if method.is_empty() {
            // Response frame; error responses map to harness error events,
            // mirroring the live reader.
            if let Some(error) = value.get("error") {
                process_rpc_error(error, &tx);
                drain_events(&mut rx, &mut events);
            }
            continue;
        }
        let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
        process_notification(method, &params, &mut state, &tx);
        drain_events(&mut rx, &mut events);
    }

    events
}

fn replay_opencode_trace(file_name: &str) -> Vec<ConversationEvent> {
    let mut lines = read_trace_lines(file_name)
        .into_iter()
        .filter(|line| !line.trim().is_empty());
    let session_create: Value = serde_json::from_str(
        &lines
            .next()
            .expect("opencode trace should start with session create payload"),
    )
    .expect("session create payload should be valid json");
    let session_id = session_create
        .get("id")
        .and_then(Value::as_str)
        .expect("session create payload should include canonical id")
        .to_string();
    let mut state = opencode_mapping::ReaderState::new(session_id);
    let mut events = Vec::new();

    for line in lines {
        let value: Value = serde_json::from_str(&line).expect("trace line should be valid json");
        let mapped = opencode_mapping::map_event(&value, &mut state);
        events.extend(mapped.events);
    }

    events
}

#[test]
fn claude_trace_normal_turn() {
    let (events, session_id) = replay_claude_trace("claude_normal_turn.ndjson");
    assert_eq!(
        session_id.as_deref(),
        Some("sess_claude_normal"),
        "system event's session id should be captured for --resume"
    );
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec!["text_delta", "turn_completed", "turn_usage"]
    );
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
}

#[test]
fn claude_trace_crash_mid_tool_marks_failed_items() {
    let (events, _session_id) = replay_claude_trace("claude_crash_mid_tool.ndjson");

    assert!(matches!(
        events.first(),
        Some(ConversationEvent::ItemStarted {
            item: ConversationItem::Command { .. },
            ..
        })
    ));

    assert!(events.iter().any(|event| {
        matches!(
            event,
            ConversationEvent::ItemCompleted {
                item: ConversationItem::Command { status, .. },
                ..
            } if *status == crate::chat::types::Lifecycle::Failed
        )
    }));

    assert!(matches!(
        events.last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Failed,
            ..
        })
    ));
}

#[test]
fn claude_trace_multi_tool_lifecycle() {
    let (events, _session_id) = replay_claude_trace("claude_multi_tool.ndjson");
    let started = events
        .iter()
        .filter(|event| matches!(event, ConversationEvent::ItemStarted { .. }))
        .count();
    let completed = events
        .iter()
        .filter(|event| matches!(event, ConversationEvent::ItemCompleted { .. }))
        .count();

    assert_eq!(started, 2);
    assert_eq!(completed, 2);
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
}

#[test]
fn codex_trace_normal_turn() {
    let events = replay_codex_trace("codex_normal_turn.jsonl");
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    // User echoes, the completed copy of streamed agent prose, and status
    // notifications produce no events; tokenUsage folds into turn_usage.
    assert_eq!(
        event_types,
        vec!["turn_started", "text_delta", "turn_completed", "turn_usage",]
    );
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
    assert!(matches!(
        events.get(1),
        Some(ConversationEvent::TextDelta { content, .. }) if content == "OK"
    ));
    assert!(matches!(
        events.last(),
        Some(ConversationEvent::TurnUsage { usage, .. })
            if usage.input_tokens == 16065
                && usage.output_tokens == 5
                && usage.cache_read_tokens == Some(9600)
    ));
}

#[test]
fn codex_trace_error_turn() {
    let events = replay_codex_trace("codex_error.jsonl");
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec!["turn_started", "error", "turn_completed", "turn_usage"]
    );
    assert!(matches!(
        events[1],
        ConversationEvent::Error { ref code, ref message, .. }
            if code == "codex_error" && message == "stream disconnected before completion"
    ));
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Failed,
            ..
        })
    ));
}

/// A `willRetry: true` error mid-turn must NOT produce a terminal Error
/// event: the vendor keeps the turn alive and retries, so the turn survives
/// to its real completion. The error surfaces non-terminally as a Thought
/// item (journaled, visible, harmless to the scheduler).
#[test]
fn codex_trace_will_retry_error_does_not_end_the_turn() {
    let events = replay_codex_trace("codex_error_will_retry.jsonl");
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec![
            "turn_started",
            "item_completed", // the retryable error, as a Thought
            "item_completed", // the real answer after the retry
            "turn_completed",
            "turn_usage",
        ],
        "no terminal error event for a retryable error"
    );
    assert!(matches!(
        &events[1],
        ConversationEvent::ItemCompleted {
            item: ConversationItem::Thought { text, .. },
            ..
        } if text.contains("will retry") && text.contains("stream disconnected")
    ));
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
}

#[test]
fn codex_rpc_error_response_maps_to_error_event() {
    // Live 0.142.5 shape: malformed requests (e.g. `content` instead of
    // `input` on turn/steer) come back as JSON-RPC error responses.
    let events = replay_codex_lines(vec![
        r#"{"error":{"code":-32600,"message":"Invalid request: missing field `input`"},"id":5}"#
            .to_string(),
    ]);

    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        ConversationEvent::Error { ref code, ref message }
            if code == "-32600" && message == "Invalid request: missing field `input`"
    ));
}

#[test]
fn opencode_trace_normal_turn() {
    let events = replay_opencode_trace("opencode_normal_turn.ndjson");
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec!["turn_started", "text_delta", "turn_completed", "turn_usage"]
    );
    assert!(matches!(
        events.last(),
        Some(ConversationEvent::TurnUsage { .. })
    ));
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
}

#[test]
fn opencode_trace_tool_lifecycle() {
    let events = replay_opencode_trace("opencode_tool_lifecycle.ndjson");
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec![
            "turn_started",
            "item_started",
            "item_completed",
            "turn_completed",
            "turn_usage"
        ]
    );
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
}

#[test]
fn opencode_trace_error_turn() {
    let events = replay_opencode_trace("opencode_error_turn.ndjson");
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec!["turn_started", "turn_completed", "turn_usage", "error"]
    );
    assert!(matches!(
        events[1],
        ConversationEvent::TurnCompleted {
            status: Lifecycle::Failed,
            ..
        }
    ));
    assert!(matches!(
        events[3],
        ConversationEvent::Error { ref code, .. } if code == "command_failed"
    ));
}
