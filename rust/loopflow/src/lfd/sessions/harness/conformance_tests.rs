use std::fs;
use std::path::Path;

use serde_json::{json, Value};
use tokio::sync::mpsc;

use super::claude_mapping::{self, ReaderState};
use super::codex_mapping::{self, ItemPhase};
use super::opencode_mapping;
use crate::lfd::sessions::types::{SessionEvent, SessionItem, TurnStatus};

fn read_trace_lines(file_name: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/lfd/sessions/harness/testdata")
        .join(file_name);
    fs::read_to_string(path)
        .expect("trace file should exist")
        .lines()
        .map(ToString::to_string)
        .collect()
}

fn drain_events(rx: &mut mpsc::UnboundedReceiver<SessionEvent>, out: &mut Vec<SessionEvent>) {
    while let Ok(event) = rx.try_recv() {
        out.push(event);
    }
}

fn replay_claude_trace(file_name: &str) -> Vec<SessionEvent> {
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

    if !saw_turn_completed {
        for item in state.drain_failed_items() {
            events.push(SessionEvent::ItemCompleted {
                turn_id: "turn_trace".to_string(),
                item,
            });
        }
        events.push(SessionEvent::TurnCompleted {
            turn_id: "turn_trace".to_string(),
            status: TurnStatus::Failed,
        });
    }

    events
}

fn resolve_turn_id(
    turn_id_from_params: Option<String>,
    current_turn_id: &Option<String>,
) -> String {
    turn_id_from_params
        .or_else(|| current_turn_id.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

fn replay_codex_trace(file_name: &str) -> Vec<SessionEvent> {
    let mut events = Vec::new();
    let mut current_turn_id: Option<String> = None;

    for line in read_trace_lines(file_name) {
        if line.trim().is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(&line).expect("trace line should be valid json");
        let method = value
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
        let turn_id_from_params = codex_mapping::extract_turn_id(&params);

        match method {
            "turn/started" => {
                let turn_id = turn_id_from_params.unwrap_or_else(|| "unknown".to_string());
                current_turn_id = Some(turn_id.clone());
                events.push(SessionEvent::TurnStarted { turn_id });
            }
            "turn/completed" => {
                let turn_id = resolve_turn_id(turn_id_from_params, &current_turn_id);
                current_turn_id = None;
                events.push(SessionEvent::TurnCompleted {
                    turn_id,
                    status: codex_mapping::map_turn_status(&params),
                });
            }
            "item/started" => {
                let turn_id = resolve_turn_id(turn_id_from_params, &current_turn_id);
                let item = codex_mapping::build_item(&params, ItemPhase::Started);
                events.push(SessionEvent::ItemStarted { turn_id, item });
            }
            "item/completed" => {
                let turn_id = resolve_turn_id(turn_id_from_params, &current_turn_id);
                let item = codex_mapping::build_item(&params, ItemPhase::Completed);
                events.push(SessionEvent::ItemCompleted { turn_id, item });
            }
            "item/agentMessage/delta" => {
                if let Some(content) = codex_mapping::text_content(&params) {
                    let turn_id = resolve_turn_id(turn_id_from_params, &current_turn_id);
                    events.push(SessionEvent::TextDelta { turn_id, content });
                }
            }
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                if let Some(content) = codex_mapping::text_content(&params) {
                    let turn_id = resolve_turn_id(turn_id_from_params, &current_turn_id);
                    events.push(SessionEvent::ReasoningDelta { turn_id, content });
                }
            }
            "item/commandExecution/outputDelta"
            | "item/fileChange/outputDelta"
            | "item/plan/delta" => {
                if let Some(data) = codex_mapping::map_item_delta(method, &params) {
                    let turn_id = resolve_turn_id(turn_id_from_params, &current_turn_id);
                    let item_id = codex_mapping::map_item_id(&params);
                    events.push(SessionEvent::ItemUpdated {
                        turn_id,
                        item_id,
                        data,
                    });
                }
            }
            "turn/diff/updated" => {
                if let Some(diff) = params
                    .get("diff")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                {
                    let turn_id = resolve_turn_id(turn_id_from_params, &current_turn_id);
                    events.push(SessionEvent::DiffUpdated { turn_id, diff });
                }
            }
            "error" => {
                events.push(SessionEvent::Error {
                    code: params
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or("codex_error")
                        .to_string(),
                    message: params
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("codex error")
                        .to_string(),
                });
            }
            _ => {}
        }
    }

    events
}

fn infer_opencode_session_id(lines: &[String]) -> String {
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(session_id) = value.get("id").and_then(Value::as_str) {
            return session_id.to_string();
        }
        if let Some(session_id) = value
            .get("properties")
            .and_then(|properties| properties.get("sessionID"))
            .and_then(Value::as_str)
        {
            return session_id.to_string();
        }
    }
    "unknown".to_string()
}

fn replay_opencode_trace(file_name: &str) -> Vec<SessionEvent> {
    let lines = read_trace_lines(file_name);
    let session_id = infer_opencode_session_id(&lines);
    let mut state = opencode_mapping::ReaderState::new(session_id);
    let mut events = Vec::new();

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line).expect("trace line should be valid json");
        let mapped = opencode_mapping::map_event(&value, &mut state);
        events.extend(mapped.events);
    }

    events
}

#[test]
fn claude_trace_normal_turn() {
    let events = replay_claude_trace("claude_normal_turn.ndjson");
    let event_types: Vec<_> = events.iter().map(SessionEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec!["provider_session_id", "text_delta", "turn_completed"]
    );
    assert!(matches!(
        events.last(),
        Some(SessionEvent::TurnCompleted {
            status: TurnStatus::Completed,
            ..
        })
    ));
}

#[test]
fn claude_trace_crash_mid_tool_marks_failed_items() {
    let events = replay_claude_trace("claude_crash_mid_tool.ndjson");

    assert!(matches!(
        events.first(),
        Some(SessionEvent::ItemStarted {
            item: SessionItem::Command { .. },
            ..
        })
    ));

    assert!(events.iter().any(|event| {
        matches!(
            event,
            SessionEvent::ItemCompleted {
                item: SessionItem::Command { status, .. },
                ..
            } if *status == crate::lfd::sessions::types::ItemStatus::Failed
        )
    }));

    assert!(matches!(
        events.last(),
        Some(SessionEvent::TurnCompleted {
            status: TurnStatus::Failed,
            ..
        })
    ));
}

#[test]
fn claude_trace_multi_tool_lifecycle() {
    let events = replay_claude_trace("claude_multi_tool.ndjson");
    let started = events
        .iter()
        .filter(|event| matches!(event, SessionEvent::ItemStarted { .. }))
        .count();
    let completed = events
        .iter()
        .filter(|event| matches!(event, SessionEvent::ItemCompleted { .. }))
        .count();

    assert_eq!(started, 2);
    assert_eq!(completed, 2);
    assert!(matches!(
        events.last(),
        Some(SessionEvent::TurnCompleted {
            status: TurnStatus::Completed,
            ..
        })
    ));
}

#[test]
fn codex_trace_normal_turn() {
    let events = replay_codex_trace("codex_normal_turn.jsonl");
    let event_types: Vec<_> = events.iter().map(SessionEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec![
            "turn_started",
            "item_started",
            "text_delta",
            "item_completed",
            "turn_completed"
        ]
    );
    assert!(matches!(
        events.last(),
        Some(SessionEvent::TurnCompleted {
            status: TurnStatus::Completed,
            ..
        })
    ));
}

#[test]
fn codex_trace_error_turn() {
    let events = replay_codex_trace("codex_error.jsonl");
    let event_types: Vec<_> = events.iter().map(SessionEvent::event_type).collect();
    assert_eq!(event_types, vec!["turn_started", "error", "turn_completed"]);
    assert!(matches!(
        events[1],
        SessionEvent::Error { ref code, .. } if code == "codex_internal"
    ));
    assert!(matches!(
        events.last(),
        Some(SessionEvent::TurnCompleted {
            status: TurnStatus::Failed,
            ..
        })
    ));
}

#[test]
fn opencode_trace_normal_turn() {
    let events = replay_opencode_trace("opencode_normal_turn.ndjson");
    let event_types: Vec<_> = events.iter().map(SessionEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec!["turn_started", "text_delta", "turn_completed"]
    );
    assert!(matches!(
        events.last(),
        Some(SessionEvent::TurnCompleted {
            status: TurnStatus::Completed,
            ..
        })
    ));
}

#[test]
fn opencode_trace_tool_lifecycle() {
    let events = replay_opencode_trace("opencode_tool_lifecycle.ndjson");
    let event_types: Vec<_> = events.iter().map(SessionEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec![
            "turn_started",
            "item_started",
            "item_completed",
            "turn_completed"
        ]
    );
    assert!(matches!(
        events.last(),
        Some(SessionEvent::TurnCompleted {
            status: TurnStatus::Completed,
            ..
        })
    ));
}

#[test]
fn opencode_trace_error_turn() {
    let events = replay_opencode_trace("opencode_error_turn.ndjson");
    let event_types: Vec<_> = events.iter().map(SessionEvent::event_type).collect();
    assert_eq!(event_types, vec!["turn_started", "turn_completed", "error"]);
    assert!(matches!(
        events[1],
        SessionEvent::TurnCompleted {
            status: TurnStatus::Failed,
            ..
        }
    ));
    assert!(matches!(
        events[2],
        SessionEvent::Error { ref code, .. } if code == "command_failed"
    ));
}
