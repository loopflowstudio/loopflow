use crate::chat::{
    parse_send_message_args, validate_turn_completion, AgentEvent, ChatTurnRequest, ChatTurnResult,
    CompletionError, ContextSnapshot, MemoryEditLog, ToolCallLog, UserMessagePhase,
    WorkspaceSnapshot,
};

fn message(content: &str, phase: UserMessagePhase) -> AgentEvent {
    AgentEvent::Message {
        content: content.to_string(),
        phase,
    }
}

// --- SendMessageArgs ---

#[test]
fn send_message_args_parse_valid_payload() {
    let payload = r#"{"content":"done","phase":"final"}"#;
    let args = parse_send_message_args(payload).unwrap();

    assert_eq!(args.content, "done");
    assert_eq!(args.phase, UserMessagePhase::Final);
}

#[test]
fn send_message_args_reject_missing_phase() {
    let payload = r#"{"content":"done"}"#;

    assert!(parse_send_message_args(payload).is_err());
}

#[test]
fn send_message_args_reject_invalid_phase() {
    let payload = r#"{"content":"done","phase":"unknown"}"#;

    assert!(parse_send_message_args(payload).is_err());
}

// --- AgentEvent round-trips ---

#[test]
fn agent_event_message_round_trips() {
    let event = message("still working", UserMessagePhase::Progress);

    let serialized = serde_json::to_string(&event).unwrap();
    let reparsed: AgentEvent = serde_json::from_str(&serialized).unwrap();

    assert_eq!(reparsed, event);
}

#[test]
fn agent_event_tool_call_round_trips() {
    let event = AgentEvent::ToolCall {
        tool: "read_file".to_string(),
        args: serde_json::json!({"path": "/tmp/test.txt"}),
    };

    let serialized = serde_json::to_string(&event).unwrap();
    let reparsed: AgentEvent = serde_json::from_str(&serialized).unwrap();

    assert_eq!(reparsed, event);
}

#[test]
fn agent_event_tool_result_round_trips() {
    let event = AgentEvent::ToolResult {
        tool: "read_file".to_string(),
        summary: "read 42 bytes".to_string(),
    };

    let serialized = serde_json::to_string(&event).unwrap();
    let reparsed: AgentEvent = serde_json::from_str(&serialized).unwrap();

    assert_eq!(reparsed, event);
}

#[test]
fn agent_event_memory_edit_round_trips() {
    let event = AgentEvent::MemoryEdit {
        op: "upsert".to_string(),
        block: "preferences".to_string(),
        detail: "added dark mode preference".to_string(),
    };

    let serialized = serde_json::to_string(&event).unwrap();
    let reparsed: AgentEvent = serde_json::from_str(&serialized).unwrap();

    assert_eq!(reparsed, event);
}

#[test]
fn agent_event_done_round_trips() {
    let event = AgentEvent::Done {
        context: ContextSnapshot {
            memory_tokens: 100,
            history_tokens: 200,
            total_tokens: 300,
        },
    };

    let serialized = serde_json::to_string(&event).unwrap();
    let reparsed: AgentEvent = serde_json::from_str(&serialized).unwrap();

    assert_eq!(reparsed, event);
}

#[test]
fn agent_event_failed_round_trips() {
    let event = AgentEvent::Failed {
        code: "timeout".to_string(),
        message: "agent exceeded 300s limit".to_string(),
    };

    let serialized = serde_json::to_string(&event).unwrap();
    let reparsed: AgentEvent = serde_json::from_str(&serialized).unwrap();

    assert_eq!(reparsed, event);
}

// --- Payload round-trips ---

#[test]
fn workspace_snapshot_round_trips() {
    let snap = WorkspaceSnapshot {
        branch: "main".to_string(),
        head_sha_at_start: "abc123".to_string(),
    };

    let serialized = serde_json::to_string(&snap).unwrap();
    let reparsed: WorkspaceSnapshot = serde_json::from_str(&serialized).unwrap();

    assert_eq!(reparsed, snap);
}

#[test]
fn chat_turn_request_round_trips() {
    let req = ChatTurnRequest {
        wave_id: "wave-1".to_string(),
        content: "what's 2+2?".to_string(),
        token_history_budget: 4096,
    };

    let serialized = serde_json::to_string(&req).unwrap();
    let reparsed: ChatTurnRequest = serde_json::from_str(&serialized).unwrap();

    assert_eq!(reparsed, req);
}

#[test]
fn chat_turn_result_round_trips() {
    let result = ChatTurnResult {
        id: "turn-1".to_string(),
        response: "4".to_string(),
        final_message_seen: true,
        memory_edits: vec![MemoryEditLog {
            op: "upsert".to_string(),
            block: "math".to_string(),
            detail: "user likes arithmetic".to_string(),
        }],
        tool_calls: vec![ToolCallLog {
            tool: "calculate".to_string(),
            args: serde_json::json!({"expr": "2+2"}),
            result_summary: "4".to_string(),
        }],
        context: ContextSnapshot {
            memory_tokens: 50,
            history_tokens: 100,
            total_tokens: 150,
        },
        snapshot: WorkspaceSnapshot {
            branch: "main".to_string(),
            head_sha_at_start: "def456".to_string(),
        },
    };

    let serialized = serde_json::to_string(&result).unwrap();
    let reparsed: ChatTurnResult = serde_json::from_str(&serialized).unwrap();

    assert_eq!(reparsed, result);
}

#[test]
fn context_snapshot_default_is_all_zeros() {
    let snap = ContextSnapshot::default();

    assert_eq!(snap.memory_tokens, 0);
    assert_eq!(snap.history_tokens, 0);
    assert_eq!(snap.total_tokens, 0);
}

// --- Completion validation ---

#[test]
fn turn_completion_requires_exactly_one_final_message() {
    let events = vec![message("done", UserMessagePhase::Final)];

    assert_eq!(validate_turn_completion(&events), Ok(()));
}

#[test]
fn turn_completion_allows_progress_before_final_message() {
    let events = vec![
        message("working", UserMessagePhase::Progress),
        message("done", UserMessagePhase::Final),
    ];

    assert_eq!(validate_turn_completion(&events), Ok(()));
}

#[test]
fn turn_completion_fails_without_final_message() {
    let events = vec![message("working", UserMessagePhase::Progress)];

    assert_eq!(
        validate_turn_completion(&events),
        Err(CompletionError::MissingFinalMessage)
    );
}

#[test]
fn turn_completion_fails_with_multiple_final_messages() {
    let events = vec![
        message("done", UserMessagePhase::Final),
        message("still done", UserMessagePhase::Final),
    ];

    assert_eq!(
        validate_turn_completion(&events),
        Err(CompletionError::MultipleFinalMessages)
    );
}

#[test]
fn turn_completion_rejects_empty_event_stream() {
    let events: Vec<AgentEvent> = vec![];

    assert_eq!(
        validate_turn_completion(&events),
        Err(CompletionError::MissingFinalMessage)
    );
}

// --- Failed-turn validation ---

#[test]
fn failed_turn_without_final_message_is_valid() {
    let events = vec![
        message("working", UserMessagePhase::Progress),
        AgentEvent::Failed {
            code: "timeout".to_string(),
            message: "exceeded limit".to_string(),
        },
    ];

    assert_eq!(validate_turn_completion(&events), Ok(()));
}

#[test]
fn failed_turn_with_final_message_is_rejected() {
    let events = vec![
        message("done", UserMessagePhase::Final),
        AgentEvent::Failed {
            code: "timeout".to_string(),
            message: "exceeded limit".to_string(),
        },
    ];

    assert_eq!(
        validate_turn_completion(&events),
        Err(CompletionError::FinalMessageOnFailedTurn)
    );
}

// --- CompletionError Display ---

#[test]
fn completion_error_has_human_readable_display() {
    assert_eq!(
        CompletionError::MissingFinalMessage.to_string(),
        "turn completed without a final message"
    );
    assert_eq!(
        CompletionError::MultipleFinalMessages.to_string(),
        "turn emitted multiple final messages"
    );
    assert_eq!(
        CompletionError::FinalMessageOnFailedTurn.to_string(),
        "turn emitted a final message alongside a failure event"
    );
}
