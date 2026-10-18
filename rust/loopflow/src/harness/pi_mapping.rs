//! Pi 1.3 session events. A Loopflow turn ends at agent_settled, not agent_end.
use std::collections::BTreeMap;

use serde_json::Value;

use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle, TurnUsage};

#[derive(Debug, Default)]
pub(super) struct Turn {
    id: String,
    usage: TurnUsage,
    messages: usize,
    failed: bool,
    final_text: String,
    streamed_text: bool,
    tools: BTreeMap<String, ConversationItem>,
}

fn message_usage(message: &Value) -> TurnUsage {
    let usage = &message["usage"];
    let input = usage["input"].as_u64();
    let read = usage["cacheRead"].as_u64();
    let write = usage["cacheWrite"].as_u64();
    let total = input
        .zip(read)
        .zip(write)
        .and_then(|((a, b), c)| a.checked_add(b)?.checked_add(c));
    TurnUsage {
        input_tokens: input,
        output_tokens: usage["output"].as_u64(),
        total_input_tokens: total,
        peak_input_tokens: total,
        cache_read_tokens: read,
        cache_write_tokens: write,
        model: message["model"].as_str().map(str::to_owned),
        cost_usd: usage["cost"]["total"].as_f64(),
        ..Default::default()
    }
}

fn text_content(content: &Value) -> String {
    if let Some(text) = content.as_str() {
        return text.into();
    }
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|part| part["type"] == "text")
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

impl Turn {
    pub fn new(id: String) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }

    pub fn event(&mut self, event: &Value) -> Vec<ConversationEvent> {
        let mut out = Vec::new();
        match event["type"].as_str() {
            Some("message_update") => {
                let update = &event["assistantMessageEvent"];
                if let Some(content) = update["delta"].as_str() {
                    match update["type"].as_str() {
                        Some("text_delta") => {
                            self.streamed_text = true;
                            out.push(ConversationEvent::TextDelta {
                                turn_id: self.id.clone(),
                                content: content.into(),
                            });
                        }
                        Some("thinking_delta") => out.push(ConversationEvent::ReasoningDelta {
                            turn_id: self.id.clone(),
                            content: content.into(),
                        }),
                        _ => {}
                    }
                }
            }
            Some("message_end") if event["message"]["role"] == "assistant" => {
                let message = &event["message"];
                self.final_text = text_content(&message["content"]);
                if !self.streamed_text && !self.final_text.is_empty() {
                    out.push(ConversationEvent::TextDelta {
                        turn_id: self.id.clone(),
                        content: self.final_text.clone(),
                    });
                }
                self.streamed_text = false;
                self.failed = message["stopReason"] == "error";
                if self.failed {
                    out.push(ConversationEvent::Error {
                        code: "pi_model_error".into(),
                        message: message["errorMessage"]
                            .as_str()
                            .unwrap_or("Pi model request failed")
                            .into(),
                        evidence: None,
                    });
                }
                let usage = message_usage(message);
                if self.messages == 0 {
                    self.usage = usage;
                } else {
                    self.add_usage(usage);
                }
                self.messages += 1;
                for (index, part) in message["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let id = format!("{}:{}:{index}", self.id, self.messages);
                    let item = match part["type"].as_str() {
                        Some("text") => ConversationItem::Message {
                            id,
                            text: part["text"].as_str().unwrap_or_default().into(),
                            phase: Some("commentary".into()),
                        },
                        Some("thinking") => ConversationItem::Thought {
                            id,
                            text: part["thinking"].as_str().unwrap_or_default().into(),
                        },
                        _ => continue,
                    };
                    out.push(ConversationEvent::ItemCompleted {
                        turn_id: self.id.clone(),
                        item,
                    });
                }
                if self.usage.is_reported() {
                    out.push(ConversationEvent::UsageCheckpoint {
                        turn_id: self.id.clone(),
                        usage: self.usage.clone(),
                        final_receipt: false,
                    });
                }
            }
            Some("tool_execution_start") => {
                if let Some(id) = event["toolCallId"].as_str() {
                    let item = ConversationItem::Tool {
                        id: id.into(),
                        name: event["toolName"].as_str().unwrap_or("unknown").into(),
                        status: Lifecycle::Running,
                        input: event.get("args").cloned(),
                        output: None,
                    };
                    self.tools.insert(id.into(), item.clone());
                    out.push(ConversationEvent::ItemStarted {
                        turn_id: self.id.clone(),
                        item,
                    });
                }
            }
            Some("tool_execution_end") => {
                if let Some(id) = event["toolCallId"].as_str() {
                    let mut item =
                        self.tools
                            .remove(id)
                            .unwrap_or_else(|| ConversationItem::Tool {
                                id: id.into(),
                                name: event["toolName"].as_str().unwrap_or("unknown").into(),
                                status: Lifecycle::Running,
                                input: None,
                                output: None,
                            });
                    if let ConversationItem::Tool { status, output, .. } = &mut item {
                        *status = if event["isError"] == true {
                            Lifecycle::Failed
                        } else {
                            Lifecycle::Completed
                        };
                        *output = Some(text_content(&event["result"]["content"]));
                    }
                    out.push(ConversationEvent::ItemCompleted {
                        turn_id: self.id.clone(),
                        item,
                    });
                }
            }
            Some("agent_settled") => {
                if self.messages == 0 && event["aborted"] != true {
                    out.push(ConversationEvent::Error {
                        code: "pi_empty_turn".into(),
                        message: "Pi settled without an assistant response".into(),
                        evidence: None,
                    });
                }
                let status = if event["aborted"] == true {
                    Lifecycle::Interrupted
                } else if self.failed || self.messages == 0 {
                    Lifecycle::Failed
                } else {
                    Lifecycle::Completed
                };
                out.extend(self.finish(status));
            }
            _ => {}
        }
        out
    }

    fn add_usage(&mut self, usage: TurnUsage) {
        // A missing receipt makes the aggregate unknown, never zero.
        let sum = |a: Option<u64>, b: Option<u64>| a.zip(b).and_then(|(a, b)| a.checked_add(b));
        self.usage.input_tokens = sum(self.usage.input_tokens, usage.input_tokens);
        self.usage.output_tokens = sum(self.usage.output_tokens, usage.output_tokens);
        self.usage.total_input_tokens =
            sum(self.usage.total_input_tokens, usage.total_input_tokens);
        self.usage.cache_read_tokens = sum(self.usage.cache_read_tokens, usage.cache_read_tokens);
        self.usage.cache_write_tokens =
            sum(self.usage.cache_write_tokens, usage.cache_write_tokens);
        self.usage.peak_input_tokens = self
            .usage
            .peak_input_tokens
            .zip(usage.peak_input_tokens)
            .map(|(a, b)| a.max(b));
        self.usage.cost_usd = self.usage.cost_usd.zip(usage.cost_usd).map(|(a, b)| a + b);
        self.usage.model = usage.model;
    }

    pub fn finish(&mut self, status: Lifecycle) -> Vec<ConversationEvent> {
        let mut out = Vec::new();
        if !self.final_text.is_empty() {
            out.push(ConversationEvent::ItemCompleted {
                turn_id: self.id.clone(),
                item: ConversationItem::Message {
                    id: format!("{}:final", self.id),
                    text: self.final_text.clone(),
                    phase: Some("final".into()),
                },
            });
        }
        for (_, mut item) in std::mem::take(&mut self.tools) {
            if let ConversationItem::Tool {
                status: item_status,
                ..
            } = &mut item
            {
                *item_status = status;
            }
            out.push(ConversationEvent::ItemCompleted {
                turn_id: self.id.clone(),
                item,
            });
        }
        if self.usage.is_reported() {
            out.push(ConversationEvent::UsageCheckpoint {
                turn_id: self.id.clone(),
                usage: self.usage.clone(),
                final_receipt: true,
            });
        }
        out.push(ConversationEvent::TurnCompleted {
            turn_id: self.id.clone(),
            status,
        });
        out
    }
}
