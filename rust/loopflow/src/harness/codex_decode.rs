//! Codex app-server (codex-cli 0.142.5) messages, decoded once each. Shapes
//! verified against a live session and the bundle from
//! `codex app-server generate-json-schema` (v2).

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use super::fact::{self, Decoded, ProviderFact, ProviderTurn, Signal};
use crate::chat::types::{
    ConversationEvent, ConversationItem, FileEdit, ItemDelta, Lifecycle, TurnUsage,
};
use crate::harness::lf_tag::LfTagParser;
use crate::id::AgentSessionId;
use crate::provider_account::RateLimitSignal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemPhase {
    Started,
    Completed,
}

/// Turn id from notification params: item/turn-scoped notifications carry a
/// top-level `turnId`; turn/started and turn/completed nest it as `turn.id`.
pub(super) fn extract_turn_id(params: &Value) -> Option<String> {
    params
        .get("turnId")
        .and_then(Value::as_str)
        .or_else(|| {
            params
                .get("turn")
                .and_then(|t| t.get("id"))
                .and_then(Value::as_str)
        })
        .map(ToString::to_string)
}

/// Vendor thread id from a `thread/started` notification's params or a
/// `thread/start` response's result — both nest it as `thread.id`.
pub(super) fn extract_thread_id(value: &Value) -> Option<AgentSessionId> {
    value
        .get("thread")
        .and_then(|t| t.get("id"))
        .and_then(Value::as_str)
        .map(AgentSessionId::from)
}

/// TurnStatus from turn/completed params: completed | interrupted | failed.
pub(super) fn map_turn_status(params: &Value) -> Lifecycle {
    match params
        .pointer("/turn/status")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "interrupted" => Lifecycle::Interrupted,
        "failed" => Lifecycle::Failed,
        _ => Lifecycle::Completed,
    }
}

/// Usage from thread/tokenUsage/updated params. `total` is cumulative for the
/// thread; the wave pipeline is turn-grained, so the latest snapshot before
/// turn/completed is what gets reported for the turn.
pub(super) fn map_token_usage(params: &Value) -> TurnUsage {
    let total = params.pointer("/tokenUsage/total");
    let last = params.pointer("/tokenUsage/last");
    let field = |key: &str| total.and_then(|t| t.get(key)).and_then(Value::as_u64);
    let peak_input_tokens = last
        .and_then(|usage| usage.get("inputTokens"))
        .and_then(Value::as_u64);
    TurnUsage {
        input_tokens: field("inputTokens"),
        output_tokens: field("outputTokens"),
        total_input_tokens: field("inputTokens"),
        peak_input_tokens,
        context_window_tokens: params
            .pointer("/tokenUsage/modelContextWindow")
            .and_then(Value::as_u64),
        reasoning_tokens: field("reasoningOutputTokens"),
        cache_read_tokens: field("cachedInputTokens"),
        cache_write_tokens: None,
        model: None,
        cost_usd: None,
    }
}

/// Name a codex rate-limit window by its duration: anything under a day is
/// the rolling session window, the rest is the weekly cap.
pub(crate) fn window_name(duration_mins: Option<u64>) -> &'static str {
    match duration_mins {
        Some(mins) if mins < 1_440 => "session",
        _ => "weekly",
    }
}

fn limit_window(value: &Value, plan: Option<&str>) -> Option<crate::store::AccountLimitWindow> {
    let used_percent = value.get("usedPercent").and_then(Value::as_u64)?;
    Some(crate::store::AccountLimitWindow {
        window: window_name(value.get("windowDurationMins").and_then(Value::as_u64)).to_string(),
        used_percent: used_percent.min(100) as u8,
        resets_at: value
            .get("resetsAt")
            .and_then(Value::as_i64)
            .map(normalize_timestamp),
        plan: plan.map(str::to_string),
    })
}

fn normalize_timestamp(timestamp: i64) -> i64 {
    if timestamp > 100_000_000_000 {
        timestamp / 1000
    } else {
        timestamp
    }
}

fn rate_limit_signal(params: &Value) -> Option<RateLimitSignal> {
    let snapshot = params.get("rateLimits").unwrap_or(params);
    let reached = snapshot.get("rateLimitReachedType").and_then(Value::as_str);
    let plan = snapshot.get("planType").and_then(Value::as_str);
    let primary = snapshot.get("primary").filter(|value| !value.is_null());
    let secondary = snapshot.get("secondary").filter(|value| !value.is_null());
    let individual = snapshot
        .get("individualLimit")
        .filter(|value| !value.is_null());
    let windows = [primary, secondary]
        .into_iter()
        .flatten()
        .filter_map(|window| limit_window(window, plan))
        .collect::<Vec<_>>();
    let utilization_percent = [primary, secondary]
        .into_iter()
        .flatten()
        .filter_map(|window| window.get("usedPercent").and_then(Value::as_u64))
        .chain(
            individual
                .and_then(|limit| limit.get("remainingPercent"))
                .and_then(Value::as_u64)
                .map(|remaining| 100_u64.saturating_sub(remaining)),
        )
        .max()
        .map(|percent| percent.min(100) as u8);
    let resets_at = [primary, secondary, individual]
        .into_iter()
        .flatten()
        .filter_map(|window| window.get("resetsAt").and_then(Value::as_i64))
        .map(|timestamp| {
            if timestamp > 100_000_000_000 {
                timestamp / 1000
            } else {
                timestamp
            }
        })
        .max();
    if reached.is_none() && utilization_percent.is_none() && resets_at.is_none() {
        return None;
    }
    Some(RateLimitSignal {
        utilization_percent,
        resets_at,
        limited: reached.is_some(),
        reason: reached.unwrap_or("rate limit update").to_string(),
        windows,
    })
}

pub(super) fn map_item_id(params: &Value) -> String {
    params
        .pointer("/item/id")
        .and_then(Value::as_str)
        .or_else(|| params.get("itemId").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string()
}

pub(super) fn map_item_type(params: &Value) -> &str {
    params
        .pointer("/item/type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

pub(super) fn build_item(params: &Value, phase: ItemPhase) -> ConversationItem {
    let id = map_item_id(params);
    let item_type = map_item_type(params);
    let item = params.get("item").unwrap_or(params);
    let status = map_item_status(item);
    let completed = phase == ItemPhase::Completed;

    match item_type {
        "commandExecution" => ConversationItem::Command {
            id,
            // 0.142.5 sends the raw command line as a single string.
            command: text_field(item, "command")
                .map(|c| vec![c])
                .unwrap_or_default(),
            cwd: required_text_field(item, "cwd"),
            status,
            output: if completed {
                text_field(item, "aggregatedOutput")
            } else {
                None
            },
            exit_code: if completed {
                item.get("exitCode")
                    .and_then(Value::as_i64)
                    .map(|v| v as i32)
            } else {
                None
            },
            duration_ms: if completed {
                item.get("durationMs").and_then(Value::as_u64)
            } else {
                None
            },
        },
        "fileChange" => ConversationItem::File {
            id,
            changes: parse_file_changes(item),
            status,
        },
        "mcpToolCall" => ConversationItem::Tool {
            id,
            name: mcp_tool_name(item),
            status,
            input: item.get("arguments").cloned(),
            output: if completed {
                mcp_tool_output(item)
            } else {
                None
            },
        },
        "agentMessage" => ConversationItem::Message {
            id,
            text: required_text_field(item, "text"),
            phase: text_field(item, "phase"),
        },
        "plan" => ConversationItem::Thought {
            id,
            text: required_text_field(item, "text"),
        },
        "reasoning" => ConversationItem::Thought {
            id,
            text: string_array(item, "summary").join("\n"),
        },
        _ => ConversationItem::Tool {
            id,
            name: item_type.to_string(),
            status,
            input: None,
            output: None,
        },
    }
}

pub(super) fn map_item_delta(method: &str, params: &Value) -> Option<ItemDelta> {
    let content = delta_content(params)?;
    match method {
        "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" => {
            Some(ItemDelta::Output { content })
        }
        "item/plan/delta" => Some(ItemDelta::PlanText { content }),
        _ => None,
    }
}

/// Streaming text: every 0.142.5 delta notification carries a `delta` field.
pub(super) fn delta_content(params: &Value) -> Option<String> {
    params
        .get("delta")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn text_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn required_text_field(value: &Value, key: &str) -> String {
    text_field(value, key).unwrap_or_default()
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Item status enums (CommandExecutionStatus, PatchApplyStatus,
/// McpToolCallStatus): inProgress | completed | failed | declined.
fn map_item_status(item: &Value) -> Lifecycle {
    match item
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("inProgress")
    {
        "completed" => Lifecycle::Completed,
        "failed" => Lifecycle::Failed,
        // Declined tool calls map to Failed for now; Decisions will give
        // declined a real home on the wire.
        "declined" => Lifecycle::Failed,
        _ => Lifecycle::Running,
    }
}

fn mcp_tool_name(item: &Value) -> String {
    let tool = required_text_field(item, "tool");
    let server = required_text_field(item, "server");
    match (server.is_empty(), tool.is_empty()) {
        (true, true) => "mcp_tool_call".to_string(),
        (true, false) => tool,
        (false, true) => server,
        (false, false) => format!("{server}/{tool}"),
    }
}

/// McpToolCallResult is `{content: [MCP blocks]}`; join the text blocks.
/// McpToolCallError is `{message}`.
fn mcp_tool_output(item: &Value) -> Option<String> {
    if let Some(result) = item.get("result").filter(|v| !v.is_null()) {
        let texts: Vec<&str> = result
            .get("content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect()
            })
            .unwrap_or_default();
        if texts.is_empty() {
            return Some(result.to_string());
        }
        return Some(texts.join("\n"));
    }
    item.pointer("/error/message")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn parse_file_changes(item: &Value) -> Vec<FileEdit> {
    item.get("changes")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|c| FileEdit {
                    path: c
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    // PatchChangeKind is an object: {"type": "add"|"delete"|"update", ...}
                    kind: c
                        .pointer("/kind/type")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    diff: c
                        .get("diff")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One app-server connection's messages in both directions.
///
/// Receipts need connection-local correlation only. Native turn/start can
/// return an already active turn; only a new turn/started notification paired
/// with its reply establishes that this client's request started it. Resumed
/// history does not establish it.
#[derive(Debug, Default)]
pub(super) struct CodexDecoder {
    sequence: u64,
    /// The turn current availability describes; late events for earlier
    /// turns still enter history but cannot replace it.
    active_turn: Option<String>,
    turns: HashMap<String, Turn>,
    /// Sequence at which each outstanding turn/start request was sent.
    requests: HashMap<String, u64>,
    display: Notifications,
}

/// All connection-local evidence for one native turn. Seeing a turn before
/// submitting a request prevents that request from claiming its origin.
#[derive(Debug, Default)]
struct Turn {
    first_seen: u64,
    started: bool,
    request: Option<String>,
    attributed: bool,
    final_answer: Option<String>,
}

impl Turn {
    fn correlate(&mut self, turn: ProviderTurn, out: &mut Vec<Decoded>) {
        if self.started && !self.attributed {
            if let Some(request) = self.request.take() {
                out.push(ProviderFact::TurnAnswered { turn, request }.into());
                self.attributed = true;
            }
        }
    }
}

/// The request id whose origin Loopflow freezes before sending `rpc`.
pub(super) fn turn_request(rpc: &Value) -> Option<String> {
    (rpc["method"] == "turn/start" && !rpc["id"].is_null()).then(|| rpc["id"].to_string())
}

impl CodexDecoder {
    /// A client message on its way to the server.
    pub(super) fn outbound(&mut self, rpc: &Value) -> Vec<Decoded> {
        let mut out = fact::activity(signals(rpc, true));
        if let Some(request) = turn_request(rpc) {
            self.active_turn = None;
            out.extend(fact::activity([Signal::NewTurn]));
            self.sequence += 1;
            self.requests.insert(request, self.sequence);
        }
        out
    }

    /// A turn already in progress when this connection resumed its thread;
    /// turn-less notifications report under it.
    pub(super) fn resume_turn(&mut self, turn: Option<String>) {
        self.display.current_turn = turn;
    }

    /// A server message. `thread` is the conversation the caller already
    /// knows; messages naming another conversation enter no history.
    pub(super) fn decode(&mut self, rpc: &Value, thread: Option<&AgentSessionId>) -> Vec<Decoded> {
        let mut out = Vec::new();
        self.sequence += 1;
        let method = rpc
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = &rpc["params"];
        let result = &rpc["result"];
        let request = if rpc.get("method").is_none() {
            self.requests.remove(&rpc["id"].to_string())
        } else {
            None
        };
        if let Some(selected) = extract_thread_id(params).or_else(|| extract_thread_id(result)) {
            out.push(ProviderFact::SessionObserved(selected).into());
        }
        if method == "account/rateLimits/updated" {
            if let Some(signal) = rate_limit_signal(params) {
                out.push(ProviderFact::RateLimit(signal).into());
            }
        }
        let observed_thread = params["threadId"]
            .as_str()
            .or(result["thread"]["id"].as_str())
            .map(AgentSessionId::from);
        if let Some(thread) = thread.cloned().or(observed_thread.clone()) {
            if observed_thread.is_none_or(|observed| observed == thread) {
                self.receipts(rpc, &thread, request, &mut out);
            }
        }
        // A server request (method and id) asks the client something; it is
        // answered by the driver, not displayed.
        if !method.is_empty() && rpc.get("id").is_none() {
            process_notification(method, params, &mut self.display, &mut out);
        }
        out
    }

    fn observe(&mut self, turn: &str) -> &mut Turn {
        self.turns.entry(turn.to_owned()).or_insert_with(|| Turn {
            first_seen: self.sequence,
            ..Turn::default()
        })
    }

    fn receipts(
        &mut self,
        rpc: &Value,
        thread: &AgentSessionId,
        request: Option<u64>,
        out: &mut Vec<Decoded>,
    ) {
        let method = rpc
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = &rpc["params"];
        let result = &rpc["result"];
        let provider_turn = |turn: &str| ProviderTurn {
            thread: thread.clone(),
            turn: turn.to_owned(),
        };
        let turn = extract_turn_id(params);
        let mut signals = Vec::new();
        if method == "turn/started" {
            if let Some(turn) = turn
                .as_ref()
                .filter(|turn| !self.turns.get(*turn).is_some_and(|seen| seen.started))
            {
                self.active_turn = Some(turn.clone());
                signals.push(Signal::NewTurn);
            }
        }
        if turn.is_none() || turn == self.active_turn {
            match (
                &turn,
                method == "item/completed" && params["item"]["type"] == "agentMessage",
            ) {
                (Some(turn), true) => {
                    if let Some(text) = params["item"]["text"]
                        .as_str()
                        .filter(|text| !text.trim().is_empty())
                    {
                        signals.push(Signal::Previewed {
                            turn: provider_turn(turn),
                            source: "codex",
                            text: text.to_owned(),
                        });
                    } else {
                        signals.push(Signal::Working);
                    }
                }
                _ if signals.is_empty() => signals = self::signals(rpc, false),
                _ => {}
            }
        }
        out.extend(fact::activity(signals));
        if let Some(turn) = turn {
            let observed = self.observe(&turn);
            match method {
                "turn/started" => {
                    observed.started = true;
                    out.push(
                        ProviderFact::TurnStarted {
                            turn: provider_turn(&turn),
                        }
                        .into(),
                    );
                }
                "thread/tokenUsage/updated" => {
                    // Keep the provider's cumulative and last-request counters.
                    // Missing baseline is not permission to attribute all lifetime
                    // usage to the client which reconnected halfway through a turn.
                    out.push(
                        ProviderFact::Usage {
                            turn: provider_turn(&turn),
                            receipt: params["tokenUsage"].clone(),
                        }
                        .into(),
                    );
                }
                "item/completed" => {
                    if let Some(text) = final_text(&params["item"]) {
                        observed.final_answer = Some(text.to_owned());
                    }
                }
                "turn/completed" => {
                    let answer = observed.final_answer.take();
                    completion(thread, &params["turn"], out);
                    if let Some(text) = answer {
                        out.push(
                            ProviderFact::Output {
                                turn: provider_turn(&turn),
                                answer: json!({"text": text}),
                            }
                            .into(),
                        );
                    }
                }
                _ => {}
            }
            self.observe(&turn).correlate(provider_turn(&turn), out);
        }
        if let (Some(turn), Some(request)) = (result["turn"]["id"].as_str(), request) {
            let request_id = rpc["id"].to_string();
            let observed = self.observe(turn);
            if observed.first_seen >= request && !observed.attributed {
                observed.request.get_or_insert(request_id);
            }
            observed.correlate(provider_turn(turn), out);
        }
        if let Some(turns) = result["thread"]["turns"]
            .as_array()
            .or(result["data"].as_array())
        {
            for turn in turns {
                if let Some(id) = turn["id"].as_str() {
                    self.observe(id);
                }
                // Snapshot recovery knows completion, not which client started the
                // turn or which AgentProcess was active at that earlier time.
                completion(thread, turn, out);
            }
        }
    }
}

fn final_text(item: &Value) -> Option<&str> {
    (item["type"] == "agentMessage" && (item["phase"].is_null() || item["phase"] == "final_answer"))
        .then(|| item["text"].as_str())
        .flatten()
}

/// Completion of a native turn object, from a live notification or snapshot.
fn completion(thread: &AgentSessionId, turn: &Value, out: &mut Vec<Decoded>) {
    let Some(id) = turn["id"].as_str() else {
        return;
    };
    let status = match turn["status"].as_str() {
        Some("completed") => Lifecycle::Completed,
        Some("failed") => Lifecycle::Failed,
        Some("interrupted") => Lifecycle::Interrupted,
        _ => return,
    };
    let provider_turn = ProviderTurn {
        thread: thread.clone(),
        turn: id.to_owned(),
    };
    if let Some(text) = turn["items"]
        .as_array()
        .and_then(|items| items.iter().rev().find_map(final_text))
    {
        out.push(
            ProviderFact::Output {
                turn: provider_turn.clone(),
                answer: json!({"text": text}),
            }
            .into(),
        );
    }
    out.push(
        ProviderFact::TurnCompleted {
            turn: Some(provider_turn),
            status,
            detail: json!({"status": turn["status"], "error": turn["error"], "started_at": turn["startedAt"], "completed_at": turn["completedAt"], "duration_ms": turn["durationMs"]}),
        }
        .into(),
    );
}

/// What one app-server message says about availability. A server request
/// that asks a person something stays pending until the client answers it.
fn signals(rpc: &Value, from_client: bool) -> Vec<Signal> {
    let method = rpc["method"].as_str();
    if from_client {
        return match (method, rpc.get("id")) {
            (None, Some(id)) => vec![Signal::InputResolved(id.to_string())],
            _ => Vec::new(),
        };
    }
    let Some(method) = method else {
        return Vec::new();
    };
    let params = &rpc["params"];
    if let Some(id) = rpc.get("id") {
        let asks = method.ends_with("requestApproval")
            || method.ends_with("requestUserInput")
            || method.contains("elicitation");
        return if asks {
            vec![Signal::InputRequested(id.to_string())]
        } else {
            Vec::new()
        };
    }
    let item = &params["item"];
    let conversational = matches!(
        item["type"].as_str(),
        Some("userMessage" | "agentMessage" | "reasoning" | "plan")
    );
    match method {
        "turn/started" => vec![Signal::Working],
        "turn/completed" => vec![Signal::Yielded(params["turn"]["status"] == "completed")],
        "serverRequest/resolved" => vec![Signal::InputResolved(params["requestId"].to_string())],
        "item/started" if conversational => vec![Signal::Working],
        "item/started" | "item/completed" => match item["id"].as_str() {
            Some(id) if method == "item/started" => vec![Signal::ToolStarted(id.to_owned())],
            Some(id) => vec![Signal::ToolResolved(id.to_owned())],
            None => Vec::new(),
        },
        _ if method.starts_with("item/") && method.to_ascii_lowercase().ends_with("delta") => {
            vec![Signal::Working]
        }
        _ => Vec::new(),
    }
}

/// Display state for one app-server connection's notifications.
#[derive(Debug, Default)]
struct Notifications {
    /// Live turn (from turn/started, cleared at turn/completed) that
    /// turn-less notifications report under.
    current_turn: Option<String>,
    /// Latest thread/tokenUsage/updated snapshot, reported at turn/completed.
    pending_usage: Option<TurnUsage>,
    /// Cumulative thread totals already attributed to completed turns. Codex
    /// reports lifetime-of-thread numbers; each turn reports the difference so
    /// its usage means the same thing as Claude's per-turn report. `None`
    /// until the first snapshot seeds it — a resumed thread arrives carrying
    /// history that belongs to earlier launches, not to this turn.
    reported: Option<ReportedTotals>,
    /// Codex closes each streamed agent message with the full text again.
    /// Remember which item ids already arrived as deltas so completion is a
    /// recovery fallback, not a second copy of the prose.
    streamed_agent_messages: HashSet<String>,
    reported_error: Option<String>,
    tag_parser: LfTagParser,
}

impl Notifications {
    /// Convert the latest thread-lifetime snapshot into this Turn's cumulative
    /// usage without advancing the completed-Turn baseline.
    fn current_turn_usage(&self) -> Option<TurnUsage> {
        let snapshot = self.pending_usage.as_ref()?;
        if !snapshot.is_reported() {
            return None;
        }
        let baseline = self.reported.unwrap_or_default();
        let gross_input = snapshot
            .input_tokens
            .map(|value| value.saturating_sub(baseline.gross_input));
        let output = snapshot
            .output_tokens
            .map(|value| value.saturating_sub(baseline.output));
        let reasoning = snapshot
            .reasoning_tokens
            .map(|value| value.saturating_sub(baseline.reasoning));
        let cached = snapshot
            .cache_read_tokens
            .map(|value| value.saturating_sub(baseline.cached));
        Some(TurnUsage {
            input_tokens: gross_input.map(|gross| gross.saturating_sub(cached.unwrap_or(0))),
            output_tokens: output,
            total_input_tokens: gross_input,
            peak_input_tokens: snapshot.peak_input_tokens,
            context_window_tokens: snapshot.context_window_tokens,
            reasoning_tokens: reasoning,
            cache_read_tokens: cached,
            cache_write_tokens: None,
            model: None,
            cost_usd: None,
        })
    }

    /// Finish the Turn and advance the attributed thread baseline.
    fn take_turn_usage(&mut self) -> Option<TurnUsage> {
        let usage = self.current_turn_usage()?;
        let snapshot = self.pending_usage.take()?;
        let baseline = self.reported.unwrap_or_default();
        self.reported = Some(ReportedTotals {
            gross_input: snapshot
                .input_tokens
                .unwrap_or(baseline.gross_input)
                .max(baseline.gross_input),
            output: snapshot
                .output_tokens
                .unwrap_or(baseline.output)
                .max(baseline.output),
            reasoning: snapshot
                .reasoning_tokens
                .unwrap_or(0)
                .max(baseline.reasoning),
            cached: snapshot.cache_read_tokens.unwrap_or(0).max(baseline.cached),
        });
        Some(usage)
    }

    /// On the first snapshot of the process, everything the thread consumed
    /// before this request belongs to earlier launches of a resumed session —
    /// baseline it out using the request-sized `last` report.
    fn seed_reported_baseline(&mut self, params: &Value) {
        if self.reported.is_some() {
            return;
        }
        let total = |key: &str| {
            params
                .pointer(&format!("/tokenUsage/total/{key}"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        let last = |key: &str| {
            params
                .pointer(&format!("/tokenUsage/last/{key}"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        self.reported = Some(ReportedTotals {
            gross_input: total("inputTokens").saturating_sub(last("inputTokens")),
            output: total("outputTokens").saturating_sub(last("outputTokens")),
            reasoning: total("reasoningOutputTokens").saturating_sub(last("reasoningOutputTokens")),
            cached: total("cachedInputTokens").saturating_sub(last("cachedInputTokens")),
        });
    }

    fn resolve_turn_id(&self, turn_id_from_params: Option<String>) -> String {
        turn_id_from_params
            .or_else(|| self.current_turn.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Display events for one app-server notification.
fn process_notification(
    method: &str,
    params: &Value,
    state: &mut Notifications,
    out: &mut Vec<Decoded>,
) {
    let turn_id_from_params = extract_turn_id(params);

    match method {
        "turn/started" => {
            let tid =
                turn_id_from_params.unwrap_or_else(|| format!("turn_{}", uuid::Uuid::new_v4()));
            state.reported_error = None;
            state.current_turn = Some(tid.clone());
            out.push(Decoded::from(ConversationEvent::TurnStarted {
                turn_id: tid,
            }));
        }
        "turn/completed" => {
            let tid = state.resolve_turn_id(turn_id_from_params);
            for parsed_event in state.tag_parser.finish_turn(&tid) {
                out.push(Decoded::from(parsed_event));
            }
            state.current_turn = None;
            let status = map_turn_status(params);
            if let Some(usage) = state.take_turn_usage() {
                out.push(Decoded::from(ConversationEvent::UsageCheckpoint {
                    turn_id: tid.clone(),
                    usage,
                    final_receipt: true,
                }));
            }
            if status == Lifecycle::Failed {
                if let Some(message) = params
                    .pointer("/turn/error/message")
                    .and_then(Value::as_str)
                    .filter(|message| state.reported_error.as_deref() != Some(*message))
                {
                    out.push(Decoded::from(ConversationEvent::Error {
                        code: "codex_error".into(),
                        message: message.to_owned(),
                        evidence: None,
                    }));
                }
            }
            out.push(Decoded::from(ConversationEvent::TurnCompleted {
                turn_id: tid,
                status,
            }));
        }
        "thread/tokenUsage/updated" => {
            // Usage arrives mid-turn as cumulative snapshots. Keep the latest
            // lifetime totals and the highest single-request window pressure.
            state.seed_reported_baseline(params);
            let mut latest = map_token_usage(params);
            if let Some(previous) = state.pending_usage.take() {
                retain_higher_context_pressure(&mut latest, &previous);
            }
            state.pending_usage = Some(latest);
            if let (Some(turn_id), Some(usage)) =
                (state.current_turn.clone(), state.current_turn_usage())
            {
                out.push(Decoded::from(ConversationEvent::UsageCheckpoint {
                    turn_id,
                    usage,
                    final_receipt: false,
                }));
            }
        }
        "item/started" | "item/completed" => {
            // The server echoes the client's own input (turn/start and
            // turn/steer text) back as userMessage items; the caller already
            // knows what it sent, so don't surface those as items.
            let item_type = map_item_type(params);
            if item_type == "userMessage" {
                return;
            }
            // agentMessage/delta is the live prose stream. Preserve a matching
            // final-answer completion as a durable phase receipt; the shared
            // turn fold recognizes that its prose was already streamed.
            if item_type == "agentMessage" {
                if method == "item/started" {
                    return;
                }
                if state.streamed_agent_messages.contains(&map_item_id(params)) {
                    let item = build_item(params, ItemPhase::Completed);
                    if matches!(
                        &item,
                        ConversationItem::Message { phase, .. }
                            if phase.as_deref() == Some("final_answer")
                    ) {
                        let tid = state.resolve_turn_id(turn_id_from_params);
                        out.push(ConversationEvent::ItemCompleted { turn_id: tid, item }.into());
                    }
                    return;
                }
            }
            let tid = state.resolve_turn_id(turn_id_from_params);
            if method == "item/started" {
                let item = build_item(params, ItemPhase::Started);
                out.push(Decoded::from(ConversationEvent::ItemStarted {
                    turn_id: tid,
                    item,
                }));
            } else {
                let item = build_item(params, ItemPhase::Completed);
                out.push(Decoded::from(ConversationEvent::ItemCompleted {
                    turn_id: tid,
                    item,
                }));
            }
        }
        "item/agentMessage/delta" => {
            if let Some(content) = delta_content(params) {
                state.streamed_agent_messages.insert(map_item_id(params));
                let tid = state.resolve_turn_id(turn_id_from_params);
                for parsed_event in state.tag_parser.consume_text(&tid, &content) {
                    out.push(Decoded::from(parsed_event));
                }
            }
        }
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
            if let Some(content) = delta_content(params) {
                let tid = state.resolve_turn_id(turn_id_from_params);
                out.push(Decoded::from(ConversationEvent::ReasoningDelta {
                    turn_id: tid,
                    content,
                }));
            }
        }
        "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" | "item/plan/delta" => {
            if let Some(data) = map_item_delta(method, params) {
                let tid = state.resolve_turn_id(turn_id_from_params);
                let item_id = map_item_id(params);
                out.push(Decoded::from(ConversationEvent::ItemUpdated {
                    turn_id: tid,
                    item_id,
                    data,
                }));
            }
        }
        "turn/diff/updated" => {
            if let Some(diff) = params
                .get("diff")
                .and_then(Value::as_str)
                .map(ToString::to_string)
            {
                let tid = state.resolve_turn_id(turn_id_from_params);
                out.push(Decoded::from(ConversationEvent::DiffUpdated {
                    turn_id: tid,
                    diff,
                }));
            }
        }
        "error" => {
            // ErrorNotification: {threadId, turnId, error: TurnError, willRetry}.
            let message = params
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("codex error")
                .to_string();
            let will_retry = params
                .get("willRetry")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if will_retry {
                // The vendor keeps the turn alive and retries on its own. A
                // terminal Error here would finalize a turn that is still
                // running: the next send_input becomes turn/steer into a
                // "failed" turn, the real turn/completed then finds nothing
                // open, and the scheduler wedges (verified cascade). Surface
                // the error non-terminally instead — it still lands in the
                // journal as a turn item.
                tracing::warn!(message, "codex reported a retryable error; turn continues");
                let tid = state.resolve_turn_id(turn_id_from_params);
                out.push(Decoded::from(ConversationEvent::ItemCompleted {
                    turn_id: tid,
                    item: ConversationItem::Thought {
                        id: format!("retry_{}", uuid::Uuid::new_v4()),
                        text: format!("codex error (will retry): {message}"),
                    },
                }));
            } else {
                state.reported_error = Some(message.clone());
                out.push(Decoded::from(ConversationEvent::Error {
                    code: "codex_error".to_string(),
                    message,
                    evidence: None,
                }));
            }
        }
        // Known 0.142.5 chatter with no conversation-level meaning.
        "thread/status/changed"
        | "account/rateLimits/updated"
        | "account/updated"
        | "mcpServer/startupStatus/updated"
        | "remoteControl/status/changed" => {
            tracing::debug!(method, "ignoring codex app-server status notification");
        }
        _ => {
            // Unknown notifications silently ignored.
        }
    }
}

/// Cumulative thread totals (gross input includes cache reads, as codex
/// reports them) already attributed to completed turns.
#[derive(Debug, Default, Clone, Copy)]
struct ReportedTotals {
    gross_input: u64,
    output: u64,
    reasoning: u64,
    cached: u64,
}

fn retain_higher_context_pressure(latest: &mut TurnUsage, previous: &TurnUsage) {
    let Some(previous_peak) = previous.peak_input_tokens else {
        return;
    };
    let Some(previous_window) = previous.context_window_tokens else {
        return;
    };
    let latest_is_higher = match (latest.peak_input_tokens, latest.context_window_tokens) {
        (Some(peak), Some(window)) => {
            u128::from(peak) * u128::from(previous_window)
                >= u128::from(previous_peak) * u128::from(window)
        }
        _ => false,
    };
    if !latest_is_higher {
        latest.peak_input_tokens = Some(previous_peak);
        latest.context_window_tokens = Some(previous_window);
    }
}

/// The display error for a JSON-RPC error response
/// (`{"error":{"code":-32600,"message":..},"id":N}`) no caller awaits.
pub(super) fn rpc_error(error: &Value) -> ConversationEvent {
    let code = error
        .get("code")
        .and_then(Value::as_i64)
        .map(|c| c.to_string())
        .unwrap_or_else(|| "codex_error".to_string());
    ConversationEvent::Error {
        code,
        message: error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("codex rpc error")
            .to_string(),
        evidence: None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A recorded trace decoded alone, without a store, as its fact sequence.
    fn replay(file: &str) -> Vec<String> {
        let mut decoder = CodexDecoder::default();
        let decoded: Vec<Decoded> = fact::trace(file)
            .iter()
            .flat_map(|rpc| decoder.decode(rpc, None))
            .collect();
        fact::fact_lines(&decoded)
    }

    /// `<fact> <thread>/<turn>` for the recorded thread.
    fn at(fact: &str, turn: &str, rest: &str) -> String {
        format!("{fact} 0197f00d-aaaa-7000-8000-000000000001/0197f00d-{turn}{rest}")
    }

    /// Usage arrives before completion; the answer follows it.
    #[test]
    fn trace_codex_normal_turn() {
        let turn = "bbbb-7000-8000-000000000002";
        assert_eq!(
            replay("codex_normal_turn.jsonl"),
            [
                "SessionObserved 0197f00d-aaaa-7000-8000-000000000001".to_owned(),
                "Activity NewTurn".to_owned(),
                at("TurnStarted", turn, ""),
                "Activity Working".to_owned(),
                r#"Activity ToolResolved("0197f00d-cccc-7000-8000-000000000003")"#.to_owned(),
                "Activity Working".to_owned(),
                "Activity Working".to_owned(),
                r#"Activity Previewed "OK""#.to_owned(),
                at("Usage", turn, ""),
                "RateLimit".to_owned(),
                "Activity Yielded(true)".to_owned(),
                at("TurnCompleted", turn, " Completed"),
                at("Output", turn, r#" {"text":"OK"}"#),
            ]
        );
    }

    #[test]
    fn trace_codex_error() {
        let turn = "dddd-7000-8000-000000000005";
        assert_eq!(
            replay("codex_error.jsonl"),
            [
                "Activity NewTurn".to_owned(),
                at("TurnStarted", turn, ""),
                "Activity Yielded(false)".to_owned(),
                at("TurnCompleted", turn, " Failed"),
            ]
        );
    }

    /// A retried error is no fact; the turn recovers and completes.
    #[test]
    fn trace_codex_error_will_retry() {
        let turn = "eeee-7000-8000-000000000006";
        assert_eq!(
            replay("codex_error_will_retry.jsonl"),
            [
                "Activity NewTurn".to_owned(),
                at("TurnStarted", turn, ""),
                r#"Activity Previewed "Recovered after the retry.""#.to_owned(),
                at("Usage", turn, ""),
                "Activity Yielded(true)".to_owned(),
                at("TurnCompleted", turn, " Completed"),
                at("Output", turn, r#" {"text":"Recovered after the retry."}"#),
            ]
        );
    }

    fn display(decoder: &mut CodexDecoder, method: &str, params: Value) -> Vec<ConversationEvent> {
        decoder
            .decode(&json!({"method": method, "params": params}), None)
            .into_iter()
            .filter_map(|decoded| match decoded {
                Decoded::Display(event) => Some(event),
                Decoded::Fact(_) => None,
            })
            .collect()
    }

    /// Codex reports cumulative thread totals; each completed turn must
    /// report only its own spend, with input net of cache reads.
    #[test]
    fn a_second_turn_reports_only_its_own_spend() {
        let mut state = Notifications::default();
        let usage = |gross, output, cached| {
            json!({
                "tokenUsage": {
                    "total": {"inputTokens": gross, "outputTokens": output,
                              "cachedInputTokens": cached, "reasoningOutputTokens": 0},
                    "last": {"inputTokens": gross},
                    "modelContextWindow": 200_000
                }
            })
        };

        state.pending_usage = Some(map_token_usage(&usage(16_065, 5, 9_600)));
        let first = state.take_turn_usage().expect("first usage");
        assert_eq!(first.input_tokens, Some(6_465));
        assert_eq!(first.total_input_tokens, Some(16_065));
        assert_eq!(first.cache_read_tokens, Some(9_600));
        assert_eq!(first.output_tokens, Some(5));

        state.pending_usage = Some(map_token_usage(&usage(20_065, 12, 13_100)));
        let second = state.take_turn_usage().expect("second usage");
        assert_eq!(
            second.input_tokens,
            Some(500),
            "gross Δ4000 minus cached Δ3500"
        );
        assert_eq!(second.total_input_tokens, Some(4_000));
        assert_eq!(second.cache_read_tokens, Some(3_500));
        assert_eq!(second.output_tokens, Some(7));

        // A turn that reported no usage stays missing rather than repeating.
        assert_eq!(state.take_turn_usage(), None);
    }

    /// A resumed thread's first snapshot carries every earlier launch's
    /// tokens in `total`; only the request-sized `last` belongs to this turn.
    #[test]
    fn a_resumed_thread_baselines_out_prior_history() {
        let mut state = Notifications::default();
        let params = json!({
            "tokenUsage": {
                "total": {"inputTokens": 100_000, "outputTokens": 9_000,
                          "cachedInputTokens": 80_000, "reasoningOutputTokens": 500},
                "last": {"inputTokens": 4_000, "outputTokens": 50,
                         "cachedInputTokens": 3_000, "reasoningOutputTokens": 10},
                "modelContextWindow": 200_000
            }
        });
        state.seed_reported_baseline(&params);
        state.pending_usage = Some(map_token_usage(&params));

        let usage = state.take_turn_usage().expect("resumed usage");
        assert_eq!(usage.total_input_tokens, Some(4_000));
        assert_eq!(usage.cache_read_tokens, Some(3_000));
        assert_eq!(usage.input_tokens, Some(1_000));
        assert_eq!(usage.output_tokens, Some(50));
        assert_eq!(usage.reasoning_tokens, Some(10));
    }

    #[test]
    fn thread_start_reply_and_notification_observe_the_native_session() {
        for rpc in [
            json!({"method":"thread/started","params":{"thread":{"id":"thread_abc","sessionId":"thread_abc"}}}),
            json!({"id":2,"result":{"thread":{"id":"thread_abc","turns":[]}}}),
        ] {
            let facts: Vec<_> = CodexDecoder::default()
                .decode(&rpc, None)
                .into_iter()
                .filter_map(|decoded| decoded.fact().cloned())
                .collect();
            assert_eq!(
                facts,
                vec![ProviderFact::SessionObserved("thread_abc".into())]
            );
        }
    }

    #[test]
    fn turn_less_notifications_report_under_the_live_turn() {
        let mut decoder = CodexDecoder::default();
        let started = display(
            &mut decoder,
            "turn/started",
            json!({ "threadId": "thread_1", "turn": { "id": "turn_1", "status": "inProgress" } }),
        );
        assert!(
            matches!(&started[..], [ConversationEvent::TurnStarted { turn_id }] if turn_id == "turn_1")
        );
        let diff = display(&mut decoder, "turn/diff/updated", json!({"diff": "+a"}));
        assert!(
            matches!(&diff[..], [ConversationEvent::DiffUpdated { turn_id, .. }] if turn_id == "turn_1")
        );

        let completed = display(
            &mut decoder,
            "turn/completed",
            json!({ "threadId": "thread_1", "turn": { "id": "turn_1", "status": "interrupted", "error": null } }),
        );
        assert!(matches!(
            completed.last(),
            Some(ConversationEvent::TurnCompleted {
                status: Lifecycle::Interrupted,
                ..
            })
        ));
        let diff = display(&mut decoder, "turn/diff/updated", json!({"diff": "+a"}));
        assert!(
            matches!(&diff[..], [ConversationEvent::DiffUpdated { turn_id, .. }] if turn_id == "unknown")
        );
    }

    #[test]
    fn completed_agent_message_does_not_repeat_streamed_prose() {
        let mut decoder = CodexDecoder::default();
        let mut events = display(
            &mut decoder,
            "item/agentMessage/delta",
            json!({"turnId": "turn_1", "itemId": "msg_1", "delta": "Hello"}),
        );
        events.extend(display(
            &mut decoder,
            "item/completed",
            json!({
                "turnId": "turn_1",
                "item": {"id": "msg_1", "type": "agentMessage", "text": "Hello", "phase": "final_answer"}
            }),
        ));

        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            ConversationEvent::TextDelta { content, .. } if content == "Hello"
        ));
        assert!(matches!(
            &events[1],
            ConversationEvent::ItemCompleted {
                item: ConversationItem::Message { text, phase, .. },
                ..
            } if text == "Hello" && phase.as_deref() == Some("final_answer")
        ));
    }

    #[test]
    fn completed_agent_message_is_a_fallback_without_deltas() {
        let events = display(
            &mut CodexDecoder::default(),
            "item/completed",
            json!({
                "turnId": "turn_1",
                "item": {"id": "msg_1", "type": "agentMessage", "text": "Recovered", "phase": "final_answer"}
            }),
        );
        assert!(matches!(
            &events[..],
            [ConversationEvent::ItemCompleted {
                item: ConversationItem::Message { text, .. },
                ..
            }] if text == "Recovered"
        ));
    }

    #[test]
    fn automated_auth_requests_do_not_wait_for_input() {
        assert!(signals(
            &json!({"id": 7, "method": "account/chatgptAuthTokens/refresh", "params": {}}),
            false
        )
        .is_empty());
    }

    #[test]
    fn build_item_maps_file_change_to_file() {
        let params = json!({
            "item": {
                "id": "item_1",
                "type": "fileChange",
                "status": "completed",
                "changes": [
                    {"path": "src/main.rs", "kind": {"type": "update", "move_path": null}, "diff": "-a\n+b"}
                ]
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            ConversationItem::File {
                id,
                changes,
                status,
            } => {
                assert_eq!(id, "item_1");
                assert_eq!(status, Lifecycle::Completed);
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].path, "src/main.rs");
                assert_eq!(changes[0].kind.as_deref(), Some("update"));
            }
            other => panic!("expected file item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_agent_message_to_message() {
        let params = json!({
            "item": {
                "id": "msg_1",
                "type": "agentMessage",
                "text": "Done",
                "phase": "final_answer",
                "memoryCitation": null
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            ConversationItem::Message { id, text, phase } => {
                assert_eq!(id, "msg_1");
                assert_eq!(text, "Done");
                assert_eq!(phase.as_deref(), Some("final_answer"));
            }
            other => panic!("expected message item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_command_string_to_single_element_argv() {
        let params = json!({
            "item": {
                "id": "cmd_1",
                "type": "commandExecution",
                "command": "ls -la",
                "commandActions": [],
                "cwd": "/tmp",
                "status": "inProgress"
            }
        });

        let item = build_item(&params, ItemPhase::Started);
        match item {
            ConversationItem::Command {
                command,
                cwd,
                status,
                ..
            } => {
                assert_eq!(command, vec!["ls -la".to_string()]);
                assert_eq!(cwd, "/tmp");
                assert_eq!(status, Lifecycle::Running);
            }
            other => panic!("expected command item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_reasoning_to_thought() {
        let params = json!({
            "item": {
                "id": "rsn_1",
                "type": "reasoning",
                "summary": ["Check tests", "Then land"]
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            ConversationItem::Thought { id, text } => {
                assert_eq!(id, "rsn_1");
                assert_eq!(text, "Check tests\nThen land");
            }
            other => panic!("expected thought item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_mcp_tool_call_to_generic_tool() {
        let params = json!({
            "item": {
                "id": "item_4",
                "type": "mcpToolCall",
                "status": "completed",
                "server": "github",
                "tool": "search",
                "arguments": { "query": "regression" },
                "result": { "content": [{"type": "text", "text": "ok"}] }
            }
        });

        let item = build_item(&params, ItemPhase::Completed);
        match item {
            ConversationItem::Tool {
                id,
                name,
                status,
                input,
                output,
            } => {
                assert_eq!(id, "item_4");
                assert_eq!(name, "github/search");
                assert_eq!(status, Lifecycle::Completed);
                assert_eq!(input, Some(json!({ "query": "regression" })));
                assert_eq!(output.as_deref(), Some("ok"));
            }
            other => panic!("expected tool item, got {other:?}"),
        }
    }

    #[test]
    fn map_token_usage_reads_total_breakdown() {
        let usage = map_token_usage(&json!({
            "threadId": "t",
            "turnId": "u",
            "tokenUsage": {
                "total": {
                    "totalTokens": 16070,
                    "inputTokens": 16065,
                    "cachedInputTokens": 9600,
                    "outputTokens": 5,
                    "reasoningOutputTokens": 0
                },
                "last": {
                    "totalTokens": 16070,
                    "inputTokens": 16065,
                    "cachedInputTokens": 9600,
                    "outputTokens": 5,
                    "reasoningOutputTokens": 0
                },
                "modelContextWindow": 258400
            }
        }));

        assert_eq!(usage.input_tokens, Some(16065));
        assert_eq!(usage.total_input_tokens, Some(16065));
        assert_eq!(usage.peak_input_tokens, Some(16065));
        assert_eq!(usage.context_window_tokens, Some(258400));
        assert_eq!(usage.output_tokens, Some(5));
        assert_eq!(usage.reasoning_tokens, Some(0));
        assert_eq!(usage.cache_read_tokens, Some(9600));
    }

    #[test]
    fn rate_limit_snapshot_maps_reached_type_and_latest_reset() {
        let signal = rate_limit_signal(&json!({
            "rateLimits": {
                "primary": {"usedPercent": 40, "resetsAt": 1900000000},
                "secondary": {"usedPercent": 80, "resetsAt": 1900000300},
                "rateLimitReachedType": "rate_limit_reached"
            }
        }))
        .unwrap();

        assert!(signal.limited);
        assert_eq!(signal.utilization_percent, Some(80));
        assert_eq!(signal.resets_at, Some(1_900_000_300));
        assert_eq!(signal.reason, "rate_limit_reached");
    }

    #[test]
    fn sparse_rate_limit_snapshot_is_ignored() {
        assert!(rate_limit_signal(&json!({"rateLimits": {}})).is_none());
    }
}
