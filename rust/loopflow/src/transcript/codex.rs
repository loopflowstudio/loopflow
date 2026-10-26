//! Codex rollouts: `response_item` rows carry the conversation; `event_msg`
//! rows repeat parts of it for the UI and are skipped.

use serde_json::Value;

use super::{text_of, unix_seconds, RequestSource, Turn, Turns};

pub(super) fn turns(rows: &[Value]) -> Vec<Turn> {
    let mut turns = Turns::default();
    for row in rows {
        if row["type"] != "response_item" {
            continue;
        }
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
    turns.0
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
