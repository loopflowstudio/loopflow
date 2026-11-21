//! Antigravity's `brain/<conversation>/.system_generated/logs/transcript_full.jsonl`
//! under its home: one step per line, numbered by `step_index`.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::live::TurnState;
use super::{Calls, Launch, RequestSource, Turn, Turns};

/// Conversation `id`'s transcript in Antigravity's home. Rows carry no
/// directory, so the id alone finds it.
pub fn file(home: &Path, id: &str) -> PathBuf {
    home.join("brain")
        .join(id)
        .join(".system_generated/logs/transcript_full.jsonl")
}

/// A `USER_INPUT` row starts a turn named `step-<n>` by its `step_index`, as
/// Loopflow's own recording names it. Each `PLANNER_RESPONSE` is one model
/// call with its usage; the rows after one that calls tools are their
/// results, in order. A call with no tools hands back, until the model
/// answers again: a subagent's report or a timer arrives as a
/// `SYSTEM_MESSAGE`, which is never a request.
pub fn read(rows: &[Value]) -> Vec<Turn> {
    let mut turns = Turns::default();
    let mut calls = Calls::default();
    let mut pending = VecDeque::new();
    for row in rows {
        let id = format!("step-{}", row["step_index"].as_u64().unwrap_or(0));
        let stamp = || row["created_at"].as_str().map(str::to_owned);
        let text = |key: &str| row[key].as_str().unwrap_or_default();
        match row["type"].as_str() {
            Some("USER_INPUT") => {
                let text = request(text("content"));
                let from = if Launch::parse(text).is_some() || text.contains("<lf:") {
                    RequestSource::Injected
                } else {
                    RequestSource::Person
                };
                pending.clear();
                turns.start(Some(id), stamp());
                turns.request(stamp(), text.to_owned(), from);
            }
            Some("SYSTEM_MESSAGE") => {}
            Some("PLANNER_RESPONSE") => {
                turns.current();
                turns.thought(&id, text("thinking"));
                turns.message(&id, text("content"));
                pending.clear();
                let tools = row["tool_calls"].as_array().map_or(&[][..], Vec::as_slice);
                for (n, tool) in tools.iter().enumerate() {
                    let call = format!("{id}:{n}");
                    turns.tool(Some(&call), tool["name"].as_str(), tool["args"].clone());
                    pending.push_back(call);
                }
                let count = |key: &str| row[key].as_u64().unwrap_or(0);
                calls.add(
                    &mut turns,
                    "agy",
                    [
                        count("input_tokens"),
                        count("cache_read_tokens"),
                        0,
                        count("output_tokens"),
                        0,
                    ],
                    None,
                    row["model"].as_str(),
                );
                if tools.is_empty() {
                    turns.end(TurnState::Ended, stamp());
                }
            }
            // A tool's result: `GENERIC`, or a type named for the tool.
            Some(_) => {
                if let Some(call) = pending.pop_front() {
                    turns.resolve(&call, row["status"] == "ERROR", text("content").to_owned());
                }
            }
            None => {}
        }
    }
    turns.finish()
}

/// What the person sent: `agy` wraps it in `<USER_REQUEST>` and appends its
/// own metadata blocks.
fn request(content: &str) -> &str {
    content
        .split_once("<USER_REQUEST>")
        .and_then(|(_, rest)| rest.split_once("</USER_REQUEST>"))
        .map_or(content, |(request, _)| request)
        .trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::live::TurnUsage;
    use loopflow_model::chat::types::{ConversationItem, Lifecycle};

    /// The agent's last message, the answer a turn hands back.
    fn last_message(turn: &Turn) -> Option<&str> {
        turn.agent()
            .filter_map(|item| match item {
                ConversationItem::Message { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .last()
    }

    fn row(index: u64, kind: &str, at: &str, more: Value) -> Value {
        let mut row = serde_json::json!({"step_index":index,"type":kind,"status":"DONE",
            "created_at":format!("2026-11-20T08:41:{at}Z")});
        row.as_object_mut()
            .unwrap()
            .extend(more.as_object().unwrap().clone());
        row
    }

    fn planner(index: u64, at: &str, tokens: [u64; 3], more: Value) -> Value {
        let mut row = row(
            index,
            "PLANNER_RESPONSE",
            at,
            serde_json::json!({"source":"MODEL","model":"gemini-3.8-flash-high",
                "input_tokens":tokens[0],"cache_read_tokens":tokens[1],"output_tokens":tokens[2]}),
        );
        row.as_object_mut()
            .unwrap()
            .extend(more.as_object().unwrap().clone());
        row
    }

    /// The rows `agy` 1.5.13 wrote for one prompt that ran `echo hi`, then a
    /// second prompt, with their text shortened.
    fn conversation() -> Vec<Value> {
        let hook = serde_json::json!({"source":"SYSTEM_SDK",
            "content":"The following is a <SYSTEM_MESSAGE> not actually sent by the user."});
        vec![
            row(
                0,
                "USER_INPUT",
                "18",
                serde_json::json!({"source":"USER_EXPLICIT","content":
                "<USER_REQUEST>\nRun `echo hi` once.\n</USER_REQUEST>\n<ADDITIONAL_METADATA>\nThe current local time is: 2026-11-20T00:41:18-08:00.\n</ADDITIONAL_METADATA>"}),
            ),
            row(1, "SYSTEM_MESSAGE", "18", hook.clone()),
            planner(
                3,
                "18",
                [9565, 0, 169],
                serde_json::json!({"tool_calls":[
                {"name":"run_command","args":{"CommandLine":"echo hi"}}]}),
            ),
            row(
                4,
                "GENERIC",
                "21",
                serde_json::json!({"source":"MODEL",
                "content":"Command exited with code 0.\n\nCommand output:\nhi\r\n"}),
            ),
            row(5, "SYSTEM_MESSAGE", "22", hook),
            planner(
                6,
                "22",
                [2366, 7526, 308],
                serde_json::json!({
                "thinking":"The command ran.","content":"`echo hi` printed hi."}),
            ),
            row(
                7,
                "USER_INPUT",
                "46",
                serde_json::json!({"source":"USER_EXPLICIT",
                "content":"<USER_REQUEST>\n<lf:message>Now pwd</lf:message>\n</USER_REQUEST>"}),
            ),
        ]
    }

    #[test]
    fn an_antigravity_turn_runs_from_its_request_through_its_tool_to_its_answer() {
        let mut rows = conversation();
        let turns = read(&rows);
        assert_eq!(turns.len(), 2);
        let turn = &turns[0];
        assert_eq!(turn.id.as_deref(), Some("step-0"));
        assert_eq!(turn.state, TurnState::Ended);
        assert_eq!(turn.started_at.as_deref(), Some("2026-11-20T08:41:18Z"));
        assert_eq!(turn.ended_at.as_deref(), Some("2026-11-20T08:41:22Z"));
        assert_eq!(turn.model.as_deref(), Some("gemini-3.8-flash-high"));
        let requests = turn.requests().collect::<Vec<_>>();
        assert_eq!(requests.len(), 1, "a system message is no request");
        assert_eq!(requests[0].text, "Run `echo hi` once.");
        assert_eq!(requests[0].from, RequestSource::Person);
        assert_eq!(
            turn.usage,
            Some(TurnUsage {
                input_tokens: 19457,
                cached_input_tokens: 7526,
                output_tokens: 477,
            })
        );
        assert_eq!(turn.receipt.usage["provider"], "agy");
        assert_eq!(turn.receipt.usage["usage"]["input_tokens"], 11931);
        let said = turn.agent().collect::<Vec<_>>();
        assert!(matches!(said[0],
            ConversationItem::Tool { name, status: Lifecycle::Completed, output: Some(output), .. }
                if name == "run_command" && output.contains("hi")));
        assert_eq!(last_message(turn), Some("`echo hi` printed hi."));
        // The second request is lf's, and nothing has answered it yet.
        assert_eq!(turns[1].id.as_deref(), Some("step-7"));
        assert_eq!(turns[1].state, TurnState::Running);
        assert_eq!(
            turns[1].requests().next().unwrap().from,
            RequestSource::Injected
        );

        // A denied tool failed, and a request sent while it ran cut the turn off.
        rows.truncate(4);
        rows[3]["status"] = "ERROR".into();
        rows.push(row(
            5,
            "USER_INPUT",
            "30",
            serde_json::json!({"content":"stop"}),
        ));
        let turns = read(&rows);
        assert_eq!(turns[0].state, TurnState::Interrupted);
        assert!(matches!(
            turns[0].agent().next().unwrap(),
            ConversationItem::Tool {
                status: Lifecycle::Failed,
                ..
            }
        ));
        assert_eq!(turns[1].requests().next().unwrap().text, "stop");
    }

    /// A subagent's report arrives as a system message after the model has
    /// answered once; the turn goes on to the model's last answer.
    #[test]
    fn an_antigravity_turn_goes_on_when_the_model_answers_a_system_message() {
        let mut rows = conversation();
        rows.truncate(6);
        rows.push(row(
            7,
            "SYSTEM_MESSAGE",
            "30",
            serde_json::json!({"source":"SYSTEM",
            "content":"The subagent finished."}),
        ));
        rows.push(planner(
            8,
            "31",
            [100, 0, 10],
            serde_json::json!({"content":"All done."}),
        ));
        let turns = read(&rows);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].state, TurnState::Ended);
        assert_eq!(turns[0].ended_at.as_deref(), Some("2026-11-20T08:41:31Z"));
        assert_eq!(turns[0].usage.as_ref().unwrap().output_tokens, 487);
        assert_eq!(last_message(&turns[0]), Some("All done."));
    }
}
