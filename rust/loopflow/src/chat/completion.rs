use crate::chat::contract::{AgentEvent, UserMessagePhase};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionError {
    MissingFinalMessage,
    MultipleFinalMessages,
}

/// Validate that a successful turn emitted exactly one final message.
pub fn validate_turn_completion(events: &[AgentEvent]) -> Result<(), CompletionError> {
    match final_message_count(events) {
        1 => Ok(()),
        0 => Err(CompletionError::MissingFinalMessage),
        _ => Err(CompletionError::MultipleFinalMessages),
    }
}

/// Return true when the event is a user-visible message.
pub fn is_user_message(event: &AgentEvent) -> bool {
    matches!(event, AgentEvent::Message { .. })
}

/// Compute final-message count from event stream.
pub fn final_message_count(events: &[AgentEvent]) -> usize {
    events
        .iter()
        .filter(|event| is_final_message(event))
        .count()
}

fn is_final_message(event: &AgentEvent) -> bool {
    matches!(
        event,
        AgentEvent::Message {
            phase: UserMessagePhase::Final,
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use crate::chat::completion::{final_message_count, is_user_message};
    use crate::chat::contract::{AgentEvent, ContextSnapshot, UserMessagePhase};

    #[test]
    fn message_helpers_classify_events() {
        let events = vec![
            AgentEvent::Message {
                content: "working".to_string(),
                phase: UserMessagePhase::Progress,
            },
            AgentEvent::Done {
                context: ContextSnapshot::default(),
            },
            AgentEvent::Message {
                content: "all set".to_string(),
                phase: UserMessagePhase::Final,
            },
        ];

        assert!(is_user_message(&events[0]));
        assert!(!is_user_message(&events[1]));
        assert!(is_user_message(&events[2]));
        assert_eq!(final_message_count(&events), 1);
    }
}
