use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use super::claude_decode::ClaudeDecoder;
use super::codex_decode::{rpc_error, CodexDecoder};
use super::fact::{Decoded, ProviderFact};
use super::fact_writer::FactWriter;
use super::opencode_decode::{OpenCodeDecoder, Snapshot};
use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use crate::id::AgentSessionId;

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

fn replay_claude_trace(file_name: &str) -> (Vec<ConversationEvent>, Option<AgentSessionId>) {
    let mut events = Vec::new();
    let mut decoder = ClaudeDecoder::default();
    let mut session_id = None;
    let mut saw_turn_completed = false;

    'lines: for line in read_trace_lines(file_name) {
        if line.trim().is_empty() {
            continue;
        }
        // The decoder reports the provider turn's end; the reader (mirrored
        // here) owns the terminal TurnCompleted.
        for decoded in decoder.decode(&line, "turn_trace").unwrap() {
            match decoded {
                Decoded::Display(event) => events.push(event),
                Decoded::Fact(ProviderFact::SessionObserved(id)) => session_id = Some(id),
                Decoded::Fact(ProviderFact::TurnCompleted { status, .. }) => {
                    saw_turn_completed = true;
                    events.push(ConversationEvent::TurnCompleted {
                        turn_id: "turn_trace".to_string(),
                        status,
                    });
                    break 'lines;
                }
                Decoded::Fact(_) => {}
            }
        }
    }

    if !saw_turn_completed {
        for item in decoder.drain_open_items(Lifecycle::Failed) {
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

/// Replay codex app-server lines through the production decoder, so traces
/// pin real behavior. Unawaited error responses display as the live reader's.
fn replay_codex_lines(lines: Vec<String>) -> Vec<ConversationEvent> {
    let mut decoder = CodexDecoder::default();
    let mut events = Vec::new();

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(&line).expect("trace line should be valid json");
        events.extend(decoder.decode(&value, None).into_iter().filter_map(
            |decoded| match decoded {
                Decoded::Display(event) => Some(event),
                Decoded::Fact(_) => None,
            },
        ));
        if let (None, Some(error)) = (value.get("method"), value.get("error")) {
            events.push(rpc_error(error));
        }
    }

    events
}

#[test]
fn claude_trace_normal_turn() {
    let (events, session_id) = replay_claude_trace("claude_normal_turn.ndjson");
    assert_eq!(
        session_id,
        Some("sess_claude_normal".into()),
        "system event's session id should be captured for --resume"
    );
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec!["text_delta", "usage_checkpoint", "turn_completed"]
    );
    assert!(matches!(
        events.get(1),
        Some(ConversationEvent::UsageCheckpoint { usage, .. })
            if usage.input_tokens == Some(321)
                && usage.output_tokens == Some(123)
                && usage.total_input_tokens == Some(337)
                && usage.cache_read_tokens == Some(11)
                && usage.cache_write_tokens == Some(5)
                && usage.reasoning_tokens == Some(9)
                && usage.model.as_deref() == Some("claude-sonnet-4")
                && usage.cost_usd == Some(0.2)
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
    // User echoes and status notifications produce no events. The final-answer
    // completion preserves its phase receipt; tokenUsage folds into usage_checkpoint.
    assert_eq!(
        event_types,
        vec![
            "turn_started",
            "text_delta",
            "item_completed",
            "usage_checkpoint",
            "usage_checkpoint",
            "turn_completed",
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
    assert!(matches!(
        events.get(1),
        Some(ConversationEvent::TextDelta { content, .. }) if content == "OK"
    ));
    assert!(matches!(
        events.get(2),
        Some(ConversationEvent::ItemCompleted {
            item: ConversationItem::Message { text, phase, .. },
            ..
        }) if text == "OK" && phase.as_deref() == Some("final_answer")
    ));
    // Codex reports cumulative gross input (cache included); the harness
    // reports the turn's own spend in Claude's shape: input net of cache,
    // gross in total_input_tokens.
    assert!(matches!(
        events.get(3),
        Some(ConversationEvent::UsageCheckpoint {
            usage,
            final_receipt: false,
            ..
        })
            if usage.input_tokens == Some(6465)
                && usage.output_tokens == Some(5)
                && usage.cache_read_tokens == Some(9600)
                && usage.total_input_tokens == Some(16065)
    ));
    assert!(matches!(
        events.get(4),
        Some(ConversationEvent::UsageCheckpoint {
            final_receipt: true,
            ..
        })
    ));
}

#[test]
fn codex_trace_error_turn() {
    let events = replay_codex_trace("codex_error.jsonl");
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(event_types, vec!["turn_started", "error", "turn_completed"]);
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

#[test]
fn codex_inline_error_survives_without_a_separate_notification() {
    let lines: Vec<_> = read_trace_lines("codex_error.jsonl")
        .into_iter()
        .filter(|line| serde_json::from_str::<Value>(line).unwrap()["method"] != "error")
        .collect();
    let events = replay_codex_lines(lines.clone().into_iter().chain(lines).collect());
    let kinds: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        kinds,
        vec![
            "turn_started",
            "error",
            "turn_completed",
            "turn_started",
            "error",
            "turn_completed"
        ]
    );
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
            "usage_checkpoint",
            "usage_checkpoint",
            "turn_completed",
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
        ConversationEvent::Error { ref code, ref message, .. }
            if code == "-32600" && message == "Invalid request: missing field `input`"
    ));
}

/// Display events from one OpenCode snapshot, recording its facts.
fn opencode_display(
    decoder: &mut OpenCodeDecoder,
    writer: &mut FactWriter,
    messages: &[Value],
) -> Vec<ConversationEvent> {
    let Snapshot { decoded, .. } = decoder.messages(messages);
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(writer.write(&decoded))
        .unwrap();
    decoded
        .into_iter()
        .filter_map(|decoded| match decoded {
            Decoded::Display(event) => Some(event),
            Decoded::Fact(_) => None,
        })
        .collect()
}

// Synthetic native message shapes follow the isolated OpenCode 2.0.32 proof.
// Status/SSE alone supplies neither a native completion nor measured usage.
#[test]
fn opencode_native_history_preserves_output_tools_and_usage_missingness() {
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;

    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("store.db");
    let store = SqliteStore::open_ephemeral(&path).unwrap();
    let sql = rusqlite::Connection::open(&path).unwrap();
    let process = LfProcessId::new();
    sql.execute(
        "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
        [process.as_str()],
    )
    .unwrap();
    for measured in [None, Some(0), Some(40)] {
        let input = crate::session_record::new_artifact_key();
        let session = format!("session-{measured:?}");
        store.test_session(&session, &input);
        let attachment = store
            .claim_session_attachment(&session, None, &process, false)
            .unwrap();
        let mut writer = FactWriter::new(
            store.clone(),
            session.clone(),
            Some(attachment),
            None,
            Default::default(),
        );
        let agent_session = AgentSessionId::from(session.as_str());
        let mut decoder = OpenCodeDecoder::new(agent_session.clone(), None);
        let request = decoder.request(false);
        writer.outbound(&request).unwrap();
        let user = json!({"id":request,"type":"user","text":"request","time":{"created":0}});
        let mut message = json!({"id":"assistant","type":"assistant","time":{"created":1},
            "content":[{"id":"tool","type":"tool","name":"shell",
                "state":{"status":"running","input":{"command":"echo ok"}}}]});
        let started = opencode_display(&mut decoder, &mut writer, &[user.clone(), message.clone()]);
        assert!(matches!(&started[..], [
                ConversationEvent::TurnStarted { turn_id },
                ConversationEvent::ItemStarted {turn_id: tool_turn, item: ConversationItem::Command {status: Lifecycle::Running, ..}},
            ] if turn_id == &request && tool_turn == &request));

        assert!(store
            .session_history(&session, 0, 0)
            .unwrap()
            .iter()
            .all(|row| row.kind != crate::session::SessionEventKind::Completed));

        message["content"][0]["state"] = json!({"status":"completed","input":{"command":"echo ok"},"content":[{"type":"text","text":"ok"}],"metadata":{"exit":0},"time":{"start":1,"end":2}});
        message["content"].as_array_mut().unwrap().push(json!({"id":"answer","sessionID":session,"messageID":"assistant","type":"text","text":"Final answer"}));
        message["finish"] = json!("stop");
        message["time"]["completed"] = json!(3);
        if let Some(input) = measured {
            message["tokens"] =
                json!({"input":input,"output":5,"reasoning":0,"cache":{"read":0,"write":0}});
            message["cost"] = json!(0.5);
        }
        let messages = [user.clone(), message.clone()];
        let output = opencode_display(&mut decoder, &mut writer, &messages);
        assert!(output.iter().any(|event| matches!(event, ConversationEvent::ItemCompleted {turn_id,item:ConversationItem::Command {output:Some(text),exit_code:Some(0),..}} if turn_id == &request && text == "ok")));
        assert!(output.iter().any(|event| matches!(event, ConversationEvent::TextDelta {turn_id,content} if turn_id == &request && content == "Final answer")));
        assert!(opencode_display(&mut decoder, &mut writer, &messages).is_empty());
        let idle = json!({"id":"idle","type":"idle","outcome":"succeeded","time":{"created":4}});
        let completion = opencode_display(
            &mut decoder,
            &mut writer,
            &[user.clone(), message.clone(), idle.clone()],
        );
        assert!(
            matches!(&completion[..], [ConversationEvent::TurnCompleted {turn_id,status:Lifecycle::Completed}] if turn_id == &request)
        );
        assert!(opencode_display(&mut decoder, &mut writer, &[user, message, idle]).is_empty());
        let usage = store.input_history(input.as_str()).unwrap().usage;
        assert_eq!(usage.input_tokens, measured);
        assert_eq!(usage.output_tokens, measured.map(|_| 5));
        assert_eq!(usage.cost_usd, measured.map(|_| 0.5));
    }
}

#[test]
fn opencode_native_error_completes_only_its_request() {
    let mut decoder = OpenCodeDecoder::new("session".into(), None);
    let request = decoder.request(false);
    let user = json!({"id":request,"type":"user","text":"request"});
    let message = json!({"id":"assistant","type":"assistant","time":{"created":1,"completed":2},
        "error":{"message":"provider rejected request"},"content":[]});
    let idle = json!({"id":"idle","type":"idle","outcome":"failed","time":{"created":3},"error":{"message":"provider rejected request"}});
    let display = |decoder: &mut OpenCodeDecoder, messages: &[Value]| -> Vec<ConversationEvent> {
        decoder
            .messages(messages)
            .decoded
            .into_iter()
            .filter_map(|decoded| match decoded {
                Decoded::Display(event) => Some(event),
                Decoded::Fact(_) => None,
            })
            .collect()
    };
    let events = display(&mut decoder, &[user.clone(), message.clone(), idle.clone()]);
    assert!(
        matches!(&events[..], [ConversationEvent::TurnStarted {turn_id}, ConversationEvent::Error {message,..}, ConversationEvent::TurnCompleted {turn_id:completed,status:Lifecycle::Failed}] if turn_id == &request && completed == &request && message == "provider rejected request")
    );
    assert!(display(&mut decoder, &[user, message, idle]).is_empty());
}
