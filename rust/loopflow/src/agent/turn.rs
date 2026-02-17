use crate::agent::anthropic::{self, ContentBlock, Message, MessageContent};
use crate::agent::tools::ToolRegistry;
use crate::chat::AgentEvent;
use std::time::{Duration, Instant};

pub const DEFAULT_MAX_ITERATIONS: u32 = 200;
pub const DEFAULT_TIMEOUT_SECS: u64 = 3000;

#[derive(Debug)]
pub struct TurnConfig {
    pub max_iterations: u32,
    pub timeout: Duration,
    pub system: Option<String>,
}

impl Default for TurnConfig {
    fn default() -> Self {
        Self {
            max_iterations: DEFAULT_MAX_ITERATIONS,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            system: None,
        }
    }
}

#[derive(Debug)]
pub struct TurnResult {
    pub response: String,
    pub iterations: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub events: Vec<AgentEvent>,
}

#[derive(Debug, thiserror::Error)]
pub enum TurnError {
    #[error("max iterations ({0}) exceeded")]
    MaxIterations(u32),
    #[error("timeout ({0:?}) exceeded")]
    Timeout(Duration),
    #[error("API error: {0}")]
    Api(#[from] anthropic::ApiError),
    #[error("no text in final response")]
    NoTextResponse,
}

/// Run a single agent turn: prompt in, response out, with tool calls in between.
pub async fn run(
    prompt: &str,
    config: &TurnConfig,
    registry: &ToolRegistry,
) -> Result<TurnResult, TurnError> {
    run_with_event_handler(prompt, config, registry, |_| {}).await
}

/// Run a single turn and invoke `on_event` for each boundary event in order.
pub async fn run_with_event_handler<F>(
    prompt: &str,
    config: &TurnConfig,
    registry: &ToolRegistry,
    mut on_event: F,
) -> Result<TurnResult, TurnError>
where
    F: FnMut(&AgentEvent),
{
    let start = Instant::now();
    let tool_defs = registry.definitions();

    let mut messages: Vec<Message> = vec![Message {
        role: "user".to_string(),
        content: MessageContent::Text(prompt.to_string()),
    }];

    let mut total_input_tokens: u32 = 0;
    let mut total_output_tokens: u32 = 0;
    let mut all_events: Vec<AgentEvent> = Vec::new();

    for iteration in 1..=config.max_iterations {
        // Check timeout
        if start.elapsed() > config.timeout {
            return Err(TurnError::Timeout(config.timeout));
        }

        let request =
            anthropic::default_request(messages.clone(), tool_defs.clone(), config.system.clone());
        let response = anthropic::call(&request).await?;

        total_input_tokens += response.usage.input_tokens;
        total_output_tokens += response.usage.output_tokens;

        eprintln!(
            "[turn] iteration {iteration}: stop_reason={}, content_blocks={}",
            response.stop_reason,
            response.content.len()
        );

        // If the model is done (no more tool calls), extract text and return
        if response.stop_reason != "tool_use" {
            let text = extract_text_or_final_event(&response.content, &all_events);
            return text
                .map(|response| TurnResult {
                    response,
                    iterations: iteration,
                    input_tokens: total_input_tokens,
                    output_tokens: total_output_tokens,
                    events: all_events,
                })
                .ok_or(TurnError::NoTextResponse);
        }

        // Model wants to use tools — dispatch them through the registry
        let (tool_results, events) = make_tool_results(&response.content, registry);
        for event in events {
            on_event(&event);
            all_events.push(event);
        }

        for block in &response.content {
            if let ContentBlock::ToolUse { name, input, .. } = block {
                eprintln!("[turn]   tool_call: {name}({input})");
            }
        }
        for block in &tool_results {
            if let ContentBlock::ToolResult {
                tool_use_id,
                content,
            } = block
            {
                eprintln!("[turn]   tool_result: {tool_use_id} -> {content}");
            }
        }

        // Add the assistant's message (with tool_use blocks) to history
        messages.push(Message {
            role: "assistant".to_string(),
            content: MessageContent::Blocks(response.content),
        });

        // Add the tool results as a user message
        messages.push(Message {
            role: "user".to_string(),
            content: MessageContent::Blocks(tool_results),
        });
    }

    Err(TurnError::MaxIterations(config.max_iterations))
}

fn make_tool_results(
    assistant_content: &[ContentBlock],
    registry: &ToolRegistry,
) -> (Vec<ContentBlock>, Vec<AgentEvent>) {
    let mut blocks = Vec::new();
    let mut events = Vec::new();

    for block in assistant_content {
        if let ContentBlock::ToolUse { id, name, input } = block {
            let (output, event) = match registry.dispatch(name, input) {
                Some(result) => (result.output, result.event),
                None => (format!("unknown tool: {name}"), None),
            };
            blocks.push(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: output,
            });
            if let Some(ev) = event {
                events.push(ev);
            }
        }
    }

    (blocks, events)
}

fn extract_text(content: &[ContentBlock]) -> Option<String> {
    let texts: Vec<&str> = content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    if texts.is_empty() {
        None
    } else {
        Some(texts.join("\n"))
    }
}

fn extract_text_or_final_event(content: &[ContentBlock], events: &[AgentEvent]) -> Option<String> {
    extract_text(content).or_else(|| extract_single_final_message(events))
}

fn extract_single_final_message(events: &[AgentEvent]) -> Option<String> {
    let finals: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Message {
                content,
                phase: crate::chat::UserMessagePhase::Final,
            } => Some(content.as_str()),
            _ => None,
        })
        .collect();

    if finals.len() == 1 {
        Some(finals[0].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools;
    use crate::chat::{validate_turn_completion, UserMessagePhase};

    #[test]
    fn make_tool_results_collects_events_from_boundary_tools() {
        let registry = tools::default_registry();

        // Simulate assistant response with two tool_use blocks
        let assistant_content = vec![
            ContentBlock::ToolUse {
                id: "t1".to_string(),
                name: "send_message".to_string(),
                input: serde_json::json!({
                    "content": "Working on it...",
                    "phase": "progress"
                }),
            },
            ContentBlock::ToolUse {
                id: "t2".to_string(),
                name: "memory_edit".to_string(),
                input: serde_json::json!({
                    "op": "upsert",
                    "block": "name",
                    "detail": "Alice"
                }),
            },
        ];

        let (blocks, events) = make_tool_results(&assistant_content, &registry);

        // Two tool results returned
        assert_eq!(blocks.len(), 2);

        // Two events collected
        assert_eq!(events.len(), 2);

        // First event is a progress message
        assert!(matches!(
            &events[0],
            AgentEvent::Message {
                phase: UserMessagePhase::Progress,
                ..
            }
        ));

        // Second event is a memory edit
        assert!(matches!(&events[1], AgentEvent::MemoryEdit { .. }));
    }

    #[test]
    fn make_tool_results_no_events_from_non_boundary_tools() {
        let registry = tools::default_registry();

        let assistant_content = vec![ContentBlock::ToolUse {
            id: "t1".to_string(),
            name: "calculate".to_string(),
            input: serde_json::json!({"expression": "2 + 2"}),
        }];

        let (blocks, events) = make_tool_results(&assistant_content, &registry);

        assert_eq!(blocks.len(), 1);
        assert!(events.is_empty());
    }

    #[test]
    fn event_collection_with_final_message_passes_completion_validation() {
        let registry = tools::default_registry();

        let assistant_content = vec![ContentBlock::ToolUse {
            id: "t1".to_string(),
            name: "send_message".to_string(),
            input: serde_json::json!({
                "content": "Here's your answer!",
                "phase": "final"
            }),
        }];

        let (_blocks, events) = make_tool_results(&assistant_content, &registry);
        assert_eq!(events.len(), 1);

        // This event stream should pass completion validation
        assert!(validate_turn_completion(&events).is_ok());
    }

    #[test]
    fn jsonl_serialization_of_events() {
        let events = vec![
            AgentEvent::Message {
                content: "hello".to_string(),
                phase: UserMessagePhase::Progress,
            },
            AgentEvent::MemoryEdit {
                op: "upsert".to_string(),
                block: "name".to_string(),
                detail: "Alice".to_string(),
            },
        ];

        // Each event serializes to a single JSON line
        for event in &events {
            let json = serde_json::to_string(event).unwrap();
            assert!(
                !json.contains('\n'),
                "JSONL lines must not contain newlines"
            );
            // Round-trips
            let reparsed: AgentEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(&reparsed, event);
        }
    }

    #[test]
    fn unknown_tool_produces_no_event() {
        let registry = tools::default_registry();

        let assistant_content = vec![ContentBlock::ToolUse {
            id: "t1".to_string(),
            name: "nonexistent_tool".to_string(),
            input: serde_json::json!({}),
        }];

        let (blocks, events) = make_tool_results(&assistant_content, &registry);

        assert_eq!(blocks.len(), 1);
        if let ContentBlock::ToolResult { content, .. } = &blocks[0] {
            assert!(content.contains("unknown tool"));
        }
        assert!(events.is_empty());
    }

    #[test]
    fn extract_text_or_final_event_prefers_text_when_present() {
        let content = vec![ContentBlock::Text {
            text: "assistant text".to_string(),
        }];
        let events = vec![AgentEvent::Message {
            content: "final from event".to_string(),
            phase: UserMessagePhase::Final,
        }];

        let extracted = extract_text_or_final_event(&content, &events);
        assert_eq!(extracted.as_deref(), Some("assistant text"));
    }

    #[test]
    fn extract_text_or_final_event_falls_back_to_single_final_event() {
        let content: Vec<ContentBlock> = vec![];
        let events = vec![AgentEvent::Message {
            content: "final from event".to_string(),
            phase: UserMessagePhase::Final,
        }];

        let extracted = extract_text_or_final_event(&content, &events);
        assert_eq!(extracted.as_deref(), Some("final from event"));
    }

    #[test]
    fn extract_text_or_final_event_requires_exactly_one_final_event() {
        let content: Vec<ContentBlock> = vec![];
        let events = vec![
            AgentEvent::Message {
                content: "first".to_string(),
                phase: UserMessagePhase::Final,
            },
            AgentEvent::Message {
                content: "second".to_string(),
                phase: UserMessagePhase::Final,
            },
        ];

        let extracted = extract_text_or_final_event(&content, &events);
        assert_eq!(extracted, None);
    }
}
