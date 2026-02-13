use crate::chat::{
    parse_send_message_args, validate_turn_completion, AgentEvent, CompletionError,
    UserMessagePhase,
};

fn message(content: &str, phase: UserMessagePhase) -> AgentEvent {
    AgentEvent::Message {
        content: content.to_string(),
        phase,
    }
}

#[test]
fn agent_message_event_round_trips_phase() {
    let event = message("still working", UserMessagePhase::Progress);

    let serialized = serde_json::to_string(&event).unwrap();
    let reparsed: AgentEvent = serde_json::from_str(&serialized).unwrap();

    assert_eq!(reparsed, event);
}

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
