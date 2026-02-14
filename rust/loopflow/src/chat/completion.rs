use crate::chat::contract::{AgentEvent, UserMessagePhase};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompletionError {
    #[error("turn completed without a final message")]
    MissingFinalMessage,
    #[error("turn emitted multiple final messages")]
    MultipleFinalMessages,
    #[error("turn emitted a final message alongside a failure event")]
    FinalMessageOnFailedTurn,
}

/// Validate that a turn's event stream satisfies the completion contract.
///
/// Successful turns must emit exactly one `Message { phase: Final }`.
/// Failed turns (containing a `Failed` event) must not emit any final message.
///
/// # Errors
///
/// Returns [`CompletionError::MissingFinalMessage`] when no final message was
/// emitted on a non-failed turn, [`CompletionError::MultipleFinalMessages`]
/// when more than one final message was emitted, or
/// [`CompletionError::FinalMessageOnFailedTurn`] when a final message
/// accompanies a `Failed` event.
pub fn validate_turn_completion(events: &[AgentEvent]) -> Result<(), CompletionError> {
    let has_failure = events.iter().any(is_failed_event);
    let finals = final_message_count(events);

    if has_failure && finals > 0 {
        return Err(CompletionError::FinalMessageOnFailedTurn);
    }
    if has_failure {
        return Ok(());
    }

    match finals {
        1 => Ok(()),
        0 => Err(CompletionError::MissingFinalMessage),
        _ => Err(CompletionError::MultipleFinalMessages),
    }
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

fn is_failed_event(event: &AgentEvent) -> bool {
    matches!(event, AgentEvent::Failed { .. })
}
