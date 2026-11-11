//! Codex rollouts: `response_item` rows carry the conversation; `event_msg`
//! rows repeat parts of it for the UI and mark where each run starts and ends.

use serde_json::{json, Map, Value};

use super::live::{TurnState, TurnUsage};
use super::{text_of, unix_seconds, RequestSource, Runs, Transcript, Turns};

/// Runs are marked by `task_started`, `task_complete` and `turn_aborted`
/// events and named by their `turn_id`. `token_count` reports the
/// conversation's running total; a run's usage is the total's growth across it.
pub(super) fn read(rows: &[Value]) -> Transcript {
    let mut turns = Turns::default();
    let mut runs = Runs::default();
    let mut tally = Tally::default();
    for row in rows {
        let payload = &row["payload"];
        let stamp = || row["timestamp"].as_str().map(str::to_owned);
        match (row["type"].as_str(), payload["type"].as_str()) {
            (Some("response_item"), _) => item(&mut turns, row),
            (Some("event_msg"), Some("task_started")) => {
                runs.start(payload["turn_id"].as_str().map(str::to_owned), stamp());
                tally.start();
            }
            (Some("turn_context"), _) => {
                if let (Some(model), Some(run)) = (payload["model"].as_str(), runs.open()) {
                    run.model = Some(model.to_owned());
                }
            }
            (Some("event_msg"), Some("token_count")) => {
                let info = &payload["info"];
                if !info["total_token_usage"].is_object() {
                    continue;
                }
                // A report after a run ended moves only the running total.
                if runs.open().is_none() && !runs.runs.is_empty() {
                    tally.count(info, true);
                    continue;
                }
                let run = runs.current();
                tally.count(info, run.started_at.is_some());
                (run.usage, run.receipt) = tally.usage();
            }
            (Some("event_msg"), Some(end @ ("task_complete" | "turn_aborted"))) => {
                let state = if end == "task_complete" {
                    TurnState::Ended
                } else {
                    TurnState::Interrupted
                };
                runs.end(state, stamp());
                if let Some(run) = runs.runs.last_mut() {
                    run.duration_ms = payload["duration_ms"].as_u64();
                }
            }
            _ => {}
        }
    }
    Transcript {
        turns: turns.0,
        runs: runs.runs,
    }
}

/// The conversation's running token total, and where the current run began.
#[derive(Debug, Default)]
struct Tally {
    /// The total when the run started; absent until one is known.
    before: Option<Value>,
    total: Option<Value>,
    last: Value,
    window: Value,
    peak: u64,
}

impl Tally {
    fn start(&mut self) {
        self.before = Some(self.total.clone().unwrap_or(Value::Null));
        self.peak = 0;
    }

    /// `started` is whether the run's start was among the rows read; one
    /// begun before them counts from its first report.
    fn count(&mut self, info: &Value, started: bool) {
        let total = &info["total_token_usage"];
        let last = &info["last_token_usage"];
        if !started && self.before.is_none() {
            self.before = Some(counters(total, |key, now| {
                now.saturating_sub(last[key].as_u64().unwrap_or(0))
            }));
        }
        self.peak = self.peak.max(last["input_tokens"].as_u64().unwrap_or(0));
        self.total = Some(total.clone());
        self.last = last.clone();
        self.window = info["model_context_window"].clone();
    }

    /// The run's usage so far, and the same as the live stream's receipt:
    /// Codex's app-server names these counters in camel case.
    fn usage(&self) -> (TurnUsage, Value) {
        let total = self.total.as_ref().unwrap_or(&Value::Null);
        let before = self.before.as_ref().unwrap_or(&Value::Null);
        let grown = |key: &str| {
            let count = |from: &Value| from[key].as_u64().unwrap_or(0);
            count(total).saturating_sub(count(before))
        };
        let camel = |from: &Value| {
            Value::Object(
                counters(total, |key, _| from[key].as_u64().unwrap_or(0))
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(key, count)| (camel_case(key), count.clone()))
                    .collect::<Map<_, _>>(),
            )
        };
        (
            TurnUsage {
                input_tokens: grown("input_tokens"),
                cached_input_tokens: grown("cached_input_tokens"),
                output_tokens: grown("output_tokens"),
            },
            json!({
                "baseline": camel(before),
                "last": camel(&self.last),
                "total": camel(total),
                "modelContextWindow": self.window,
                "peakInputTokens": self.peak,
            }),
        )
    }
}

/// Each of `total`'s counters, mapped.
fn counters(total: &Value, count: impl Fn(&str, u64) -> u64) -> Value {
    Value::Object(
        total
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(key, now)| Some((key.clone(), count(key, now.as_u64()?).into())))
            .collect::<Map<_, _>>(),
    )
}

fn camel_case(snake: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in snake.chars() {
        match c {
            '_' => upper = true,
            c if upper => {
                out.extend(c.to_uppercase());
                upper = false;
            }
            c => out.push(c),
        }
    }
    out
}

fn item(turns: &mut Turns, row: &Value) {
    let item = &row["payload"];
    let id = item["id"].as_str().unwrap_or_default();
    match item["type"].as_str() {
        Some("message") => {
            let text = text_of(&item["content"]);
            match item["role"].as_str() {
                Some("user") => {
                    let from = if injected(&text) {
                        RequestSource::Injected
                    } else {
                        RequestSource::Person
                    };
                    turns.request(unix_seconds(&row["timestamp"]), text, from);
                }
                Some("assistant") => turns.message(id, &text),
                _ => {}
            }
        }
        Some("reasoning") => turns.thought(id, &text_of(&item["summary"])),
        Some("function_call") => turns.tool(
            item["call_id"].as_str(),
            item["name"].as_str(),
            Value::String(item["arguments"].as_str().unwrap_or_default().to_owned()),
        ),
        Some("custom_tool_call") => turns.tool(
            item["call_id"].as_str(),
            item["name"].as_str(),
            Value::String(item["input"].as_str().unwrap_or_default().to_owned()),
        ),
        Some("function_call_output" | "custom_tool_call_output") => {
            let output = text_of(&item["output"]);
            turns.resolve(
                item["call_id"].as_str().unwrap_or_default(),
                exited_nonzero(&output),
                output,
            );
        }
        _ => {}
    }
}

/// Instructions and context Codex and Loopflow write as user messages.
fn injected(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.is_empty()
        || trimmed.starts_with("# AGENTS.md instructions")
        || trimmed.starts_with("<environment_context>")
        || trimmed.starts_with("<user_instructions>")
        || trimmed.contains("<lf:")
}

/// Whether a Codex tool output reports a nonzero exit, in any of the shapes
/// Codex has written: `"exit_code":N`, `Exit code: N`, `exited with code N`.
fn exited_nonzero(output: &str) -> bool {
    ["\"exit_code\":", "Exit code: ", "exited with code "]
        .iter()
        .any(|marker| {
            output.match_indices(marker).any(|(at, _)| {
                let rest = output[at + marker.len()..].trim_start();
                let digits = rest
                    .char_indices()
                    .take_while(|(index, c)| c.is_ascii_digit() || (*index == 0 && *c == '-'))
                    .map(|(_, c)| c)
                    .collect::<String>();
                digits.parse::<i64>().is_ok_and(|code| code != 0)
            })
        })
}
