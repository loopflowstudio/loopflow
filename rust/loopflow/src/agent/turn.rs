use crate::agent::anthropic::{self, ContentBlock, Message, MessageContent};
use crate::agent::registry::ToolRegistry;
use std::time::{Duration, Instant};

pub const DEFAULT_MAX_ITERATIONS: u32 = 20;
pub const DEFAULT_TIMEOUT_SECS: u64 = 300;

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
    let start = Instant::now();
    let tool_defs = registry.definitions();

    let mut messages: Vec<Message> = vec![Message {
        role: "user".to_string(),
        content: MessageContent::Text(prompt.to_string()),
    }];

    let mut total_input_tokens: u32 = 0;
    let mut total_output_tokens: u32 = 0;

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
            let text = extract_text(&response.content);
            return text
                .map(|response| TurnResult {
                    response,
                    iterations: iteration,
                    input_tokens: total_input_tokens,
                    output_tokens: total_output_tokens,
                })
                .ok_or(TurnError::NoTextResponse);
        }

        // Model wants to use tools — dispatch them through the registry
        let tool_results = make_tool_results(&response.content, registry);

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
) -> Vec<ContentBlock> {
    assistant_content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => {
                let output = match registry.dispatch(name, input) {
                    Some(result) => result.output,
                    None => format!("unknown tool: {name}"),
                };
                Some(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: output,
                })
            }
            _ => None,
        })
        .collect()
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
