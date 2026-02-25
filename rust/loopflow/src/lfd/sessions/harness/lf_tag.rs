use crate::lfd::sessions::types::{SessionEvent, SuggestedActionPayload};

const SUGGEST_ACTIONS_OPEN: &str = "<lf:suggest_actions>";
const SUGGEST_ACTIONS_CLOSE: &str = "</lf:suggest_actions>";

/// Streaming parser for synthetic `<lf:...>` tag output embedded in text deltas.
#[derive(Debug, Default)]
pub(super) struct LfTagParser {
    stream_buffer: String,
    in_suggest_actions_tag: bool,
    suggest_actions_payload: String,
}

impl LfTagParser {
    pub(super) fn consume_text(&mut self, turn_id: &str, text: &str) -> Vec<SessionEvent> {
        if text.is_empty() {
            return Vec::new();
        }

        self.stream_buffer.push_str(text);
        let mut events = Vec::new();

        loop {
            if self.in_suggest_actions_tag {
                if let Some(close_idx) = self.stream_buffer.find(SUGGEST_ACTIONS_CLOSE) {
                    self.suggest_actions_payload
                        .push_str(&take_prefix(&mut self.stream_buffer, close_idx));
                    drop_prefix(&mut self.stream_buffer, SUGGEST_ACTIONS_CLOSE.len());
                    self.in_suggest_actions_tag = false;

                    if let Ok(actions) = serde_json::from_str::<Vec<SuggestedActionPayload>>(
                        self.suggest_actions_payload.trim(),
                    ) {
                        events.push(SessionEvent::SuggestedActions {
                            turn_id: turn_id.to_string(),
                            actions,
                        });
                    }
                    self.suggest_actions_payload.clear();
                    continue;
                }

                self.suggest_actions_payload.push_str(&take_stable_prefix(
                    &mut self.stream_buffer,
                    SUGGEST_ACTIONS_CLOSE,
                ));
                break;
            }

            if let Some(open_idx) = self.stream_buffer.find(SUGGEST_ACTIONS_OPEN) {
                push_text_delta(
                    &mut events,
                    turn_id,
                    take_prefix(&mut self.stream_buffer, open_idx),
                );
                drop_prefix(&mut self.stream_buffer, SUGGEST_ACTIONS_OPEN.len());
                self.in_suggest_actions_tag = true;
                continue;
            }

            push_text_delta(
                &mut events,
                turn_id,
                take_stable_prefix(&mut self.stream_buffer, SUGGEST_ACTIONS_OPEN),
            );
            break;
        }

        events
    }

    pub(super) fn finish_turn(&mut self, turn_id: &str) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        let trailing = std::mem::take(&mut self.stream_buffer);

        if self.in_suggest_actions_tag {
            let mut raw = String::from(SUGGEST_ACTIONS_OPEN);
            raw.push_str(&self.suggest_actions_payload);
            raw.push_str(&trailing);
            push_text_delta(&mut events, turn_id, raw);
        } else {
            push_text_delta(&mut events, turn_id, trailing);
        }

        self.suggest_actions_payload.clear();
        self.in_suggest_actions_tag = false;

        events
    }
}

fn take_prefix(buffer: &mut String, byte_len: usize) -> String {
    let tail = buffer.split_off(byte_len);
    let prefix = std::mem::take(buffer);
    *buffer = tail;
    prefix
}

fn drop_prefix(buffer: &mut String, byte_len: usize) {
    if byte_len > 0 {
        buffer.drain(..byte_len);
    }
}

fn take_stable_prefix(buffer: &mut String, marker: &str) -> String {
    let keep = suffix_prefix_len(buffer, marker);
    let stable_len = buffer.len().saturating_sub(keep);
    take_prefix(buffer, stable_len)
}

fn suffix_prefix_len(value: &str, marker: &str) -> usize {
    let max = std::cmp::min(value.len(), marker.len().saturating_sub(1));
    for len in (1..=max).rev() {
        if value.ends_with(&marker[..len]) {
            return len;
        }
    }
    0
}

fn push_text_delta(events: &mut Vec<SessionEvent>, turn_id: &str, content: String) {
    if content.is_empty() {
        return;
    }
    events.push(SessionEvent::TextDelta {
        turn_id: turn_id.to_string(),
        content,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_emits_text_and_suggested_actions() {
        let mut parser = LfTagParser::default();
        let events = parser.consume_text(
            "turn_1",
            "Hello<lf:suggest_actions>[{\"label\":\"Land PR\"}]</lf:suggest_actions>Done",
        );

        assert_eq!(events.len(), 3);
        assert!(matches!(
            events[0],
            SessionEvent::TextDelta { ref content, .. } if content == "Hello"
        ));
        assert!(matches!(
            events[1],
            SessionEvent::SuggestedActions { ref actions, .. } if actions.len() == 1 && actions[0].label == "Land PR"
        ));
        assert!(matches!(
            events[2],
            SessionEvent::TextDelta { ref content, .. } if content == "Done"
        ));

        let trailing = parser.finish_turn("turn_1");
        assert!(trailing.is_empty());
    }

    #[test]
    fn parser_handles_split_tag_across_chunks() {
        let mut parser = LfTagParser::default();
        let first = parser.consume_text("turn_1", "<lf:suggest_actions>[{\"label\":\"Run");
        assert!(first.is_empty());

        let second = parser.consume_text("turn_1", " tests\"}]</lf:suggest_actions>");
        assert_eq!(second.len(), 1);
        assert!(matches!(
            second[0],
            SessionEvent::SuggestedActions { ref actions, .. } if actions[0].label == "Run tests"
        ));
    }

    #[test]
    fn parser_drops_invalid_json_payload() {
        let mut parser = LfTagParser::default();
        let events = parser.consume_text(
            "turn_1",
            "<lf:suggest_actions>not-json</lf:suggest_actions>",
        );
        assert!(events.is_empty());
    }

    #[test]
    fn parser_flushes_unclosed_tag_as_text_on_finish() {
        let mut parser = LfTagParser::default();
        let _ = parser.consume_text("turn_1", "<lf:suggest_actions>[{");
        let events = parser.finish_turn("turn_1");
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            SessionEvent::TextDelta { ref content, .. } if content == "<lf:suggest_actions>[{"
        ));
    }

    #[test]
    fn parser_does_not_delay_regular_text() {
        let mut parser = LfTagParser::default();
        let events = parser.consume_text("turn_1", "hello");
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            SessionEvent::TextDelta { ref content, .. } if content == "hello"
        ));
    }
}
