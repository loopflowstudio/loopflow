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
        .filter(|event| {
            matches!(
                event,
                AgentEvent::Message {
                    phase: UserMessagePhase::Final,
                    ..
                }
            )
        })
        .count()
}

#[cfg(test)]
mod tests {
    use crate::chat::completion::{final_message_count, validate_turn_completion, CompletionError};
    use crate::chat::contract::{AgentEvent, ContextSnapshot, UserMessagePhase};

    #[test]
    fn one_final_message_is_valid_completion() {
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

        assert_eq!(final_message_count(&events), 1);
        assert_eq!(validate_turn_completion(&events), Ok(()));
    }

    #[test]
    fn no_final_message_is_invalid_completion() {
        let events = vec![
            AgentEvent::Message {
                content: "working".to_string(),
                phase: UserMessagePhase::Progress,
            },
            AgentEvent::Done {
                context: ContextSnapshot::default(),
            },
        ];

        assert_eq!(final_message_count(&events), 0);
        assert_eq!(
            validate_turn_completion(&events),
            Err(CompletionError::MissingFinalMessage)
        );
    }

    #[test]
    fn two_final_messages_is_invalid_completion() {
        let events = vec![
            AgentEvent::Message {
                content: "done".to_string(),
                phase: UserMessagePhase::Final,
            },
            AgentEvent::Message {
                content: "actually done".to_string(),
                phase: UserMessagePhase::Final,
            },
        ];

        assert_eq!(final_message_count(&events), 2);
        assert_eq!(
            validate_turn_completion(&events),
            Err(CompletionError::MultipleFinalMessages)
        );
    }
}
