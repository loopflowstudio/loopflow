//! Pi 1.3 session events, decoded once each. A Loopflow turn ends at
//! agent_settled, not agent_end. Pi evidence carries no provider-account
//! inference.
use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::{json, Value};

use super::fact::{Decoded, ProviderFact, ProviderTurn, Signal};
use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle, TurnUsage};
use crate::id::AgentSessionId;

/// One Loopflow turn in a Pi conversation.
#[derive(Debug)]
pub(super) struct Turn {
    turn: ProviderTurn,
    id: String,
    usage: TurnUsage,
    messages: usize,
    failed: bool,
    answered: bool,
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
    pub fn new(turn: ProviderTurn) -> Self {
        Self {
            id: turn.turn.clone(),
            turn,
            usage: TurnUsage::default(),
            messages: 0,
            failed: false,
            answered: false,
            final_text: String::new(),
            streamed_text: false,
            tools: BTreeMap::new(),
        }
    }

    /// Decode one session event of this turn.
    pub fn event(&mut self, event: &Value) -> Vec<Decoded> {
        let mut facts = signals(event)
            .into_iter()
            .map(|signal| ProviderFact::Activity(signal).into())
            .collect();
        let display = self.display(event, &mut facts);
        facts.extend(display.into_iter().map(Decoded::from));
        facts
    }

    /// End the turn, as Pi settled it or as its stream died.
    pub fn finish(&mut self, status: Lifecycle) -> Vec<Decoded> {
        let mut facts = Vec::new();
        let display = self.settle(status, &mut facts);
        facts.extend(display.into_iter().map(Decoded::from));
        facts
    }

    fn usage_fact(&self, final_receipt: bool) -> Decoded {
        ProviderFact::Usage {
            turn: self.turn.clone(),
            receipt: json!({"provider":"pi", "usage":self.usage, "final":final_receipt}),
        }
        .into()
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    fn display(&mut self, event: &Value, facts: &mut Vec<Decoded>) -> Vec<ConversationEvent> {
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
                self.answered =
                    message["stopReason"] == "stop" && !self.final_text.trim().is_empty();
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
                if let Some(input_tokens) = usage.total_input_tokens {
                    facts.push(ProviderFact::Request { input_tokens }.into());
                }
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
                    facts.push(self.usage_fact(false));
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
                if !self.answered && !self.failed && event["aborted"] != true {
                    out.push(ConversationEvent::Error {
                        code: "pi_empty_turn".into(),
                        message: "Pi settled without a nonempty final answer".into(),
                        evidence: None,
                    });
                }
                let status = if event["aborted"] == true {
                    Lifecycle::Interrupted
                } else if self.failed || !self.answered {
                    Lifecycle::Failed
                } else {
                    Lifecycle::Completed
                };
                out.extend(self.settle(status, facts));
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

    fn settle(&mut self, status: Lifecycle, facts: &mut Vec<Decoded>) -> Vec<ConversationEvent> {
        let mut out = Vec::new();
        facts.push(ProviderFact::Activity(Signal::Yielded(status == Lifecycle::Completed)).into());
        if !self.final_text.is_empty() {
            facts.push(
                ProviderFact::Output {
                    turn: self.turn.clone(),
                    answer: json!({"text": self.final_text}),
                }
                .into(),
            );
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
            facts.push(self.usage_fact(true));
            out.push(ConversationEvent::UsageCheckpoint {
                turn_id: self.id.clone(),
                usage: self.usage.clone(),
                final_receipt: true,
            });
        }
        facts.push(
            ProviderFact::TurnCompleted {
                turn: Some(self.turn.clone()),
                status,
                detail: json!({"status": status.name()}),
            }
            .into(),
        );
        out.push(ConversationEvent::TurnCompleted {
            turn_id: self.id.clone(),
            status,
        });
        out
    }
}

/// What one session event says about availability; settlement yields through
/// `settle`, which knows how the turn ended.
fn signals(value: &Value) -> Vec<Signal> {
    match value["type"].as_str() {
        Some("agent_start" | "message_update") => vec![Signal::Working],
        Some("tool_execution_start") => value["toolCallId"]
            .as_str()
            .map(|id| Signal::ToolStarted(id.into()))
            .into_iter()
            .collect(),
        Some("tool_execution_end") => value["toolCallId"]
            .as_str()
            .map(|id| Signal::ToolResolved(id.into()))
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// One callback from Pi's native extension, which counts its own
/// availability and supplies cumulative turn evidence.
pub(super) fn native(thread: &AgentSessionId, value: &Value) -> Result<Vec<Decoded>> {
    let turn = value["turn"].as_str();
    let require_turn = || -> Result<ProviderTurn> {
        Ok(ProviderTurn {
            thread: thread.clone(),
            turn: turn
                .ok_or_else(|| anyhow::anyhow!("Pi event has no turn"))?
                .into(),
        })
    };
    let mut out = Vec::new();
    if value["type"] == "agent_start" {
        let turn = require_turn()?;
        out.push(
            ProviderFact::TurnAnswered {
                request: turn.turn.clone(),
                turn,
            }
            .into(),
        );
    }
    let settled = value["type"] == "agent_settled";
    let mut yielded = turn.is_none() && value["type"] == "ui_prompt_end";
    if settled || (value["type"] == "message_end" && value["message"]["role"] == "assistant") {
        let mut state = Turn::new(require_turn()?);
        let mut latest = Vec::new();
        for event in value["events"].as_array().into_iter().flatten() {
            // The extension supplies cumulative evidence; retain only the latest
            // checkpoint. Existing receipt keys make repeated observations idempotent.
            latest = state.event(event);
        }
        if settled {
            latest = state.event(value);
        }
        yielded = latest.iter().any(|decoded| {
            matches!(
                decoded,
                Decoded::Fact(ProviderFact::TurnCompleted {
                    status: Lifecycle::Completed,
                    ..
                })
            )
        });
        // The extension's own counts describe availability, not replayed signals.
        out.extend(latest.into_iter().filter(|decoded| {
            matches!(
                decoded,
                Decoded::Fact(
                    ProviderFact::Output { .. }
                        | ProviderFact::Usage { .. }
                        | ProviderFact::TurnCompleted { .. }
                )
            )
        }));
    }
    let count = |key: &str| -> Result<usize> {
        Ok(value[key]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Pi callback has no {key} count"))? as usize)
    };
    out.push(
        ProviderFact::AvailabilityReading {
            open_tools: count("openTools")?,
            pending_input: count("pendingInput")?,
            yielded,
            settled,
        }
        .into(),
    );
    Ok(out)
}
