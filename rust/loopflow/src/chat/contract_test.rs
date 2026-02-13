use crate::chat::{
    parse_send_message_args, validate_turn_completion, AgentEvent, CompletionError,
    UserMessagePhase,
};

#[test]
fn agent_message_event_round_trips_phase() {
    let event = AgentEvent::Message {
        content: "still working".to_string(),
        phase: UserMessagePhase::Progress,
    };

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
fn turn_completion_requires_exactly_one_final_message() {
    let events = vec![AgentEvent::Message {
        content: "done".to_string(),
        phase: UserMessagePhase::Final,
    }];

    assert_eq!(validate_turn_completion(&events), Ok(()));
}

#[test]
fn turn_completion_fails_without_final_message() {
    let events = vec![AgentEvent::Message {
        content: "working".to_string(),
        phase: UserMessagePhase::Progress,
    }];

    assert_eq!(
        validate_turn_completion(&events),
        Err(CompletionError::MissingFinalMessage)
    );
}

#[test]
fn turn_completion_fails_with_multiple_final_messages() {
    let events = vec![
        AgentEvent::Message {
            content: "done".to_string(),
            phase: UserMessagePhase::Final,
        },
        AgentEvent::Message {
            content: "still done".to_string(),
            phase: UserMessagePhase::Final,
        },
    ];

    assert_eq!(
        validate_turn_completion(&events),
        Err(CompletionError::MultipleFinalMessages)
    );
}
