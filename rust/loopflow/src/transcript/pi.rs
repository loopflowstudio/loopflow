//! Pi session files: `sessions/--<directory>--/<time>_<id>.jsonl` under
//! Pi's home, one entry per line.

use std::collections::HashMap;

use serde_json::Value;

use super::live::TurnState;
use super::{text_of, Calls, Launch, RequestSource, Turn, Turns};

/// A user message starts a turn, named by its entry id; a Loopflow
/// `custom_message` starts one once Pi answers it. Each assistant message is
/// one model call with its own usage and cost; its `stopReason` says whether
/// the turn goes on (`toolUse`), was cut off (`aborted`, `error`) or handed
/// back. Loopflow's own recording of a Pi turn names it by Loopflow's request
/// id, which the session file never holds.
pub(super) fn read(rows: &[Value]) -> Vec<Turn> {
    let mut turns = Turns::default();
    let mut calls = Calls::default();
    for entry in branch(rows) {
        let id = entry["id"].as_str().unwrap_or_default();
        let stamp = || entry["timestamp"].as_str().map(str::to_owned);
        let message = &entry["message"];
        match (entry["type"].as_str(), message["role"].as_str()) {
            (Some("custom_message"), _) => {
                turns.request(stamp(), text_of(&entry["content"]), RequestSource::Injected);
                turns.wake(Some(id.to_owned()), stamp());
            }
            (Some("message"), Some("user")) => {
                let text = text_of(&message["content"]);
                let from = if Launch::parse(&text).is_some() || text.contains("<lf:") {
                    RequestSource::Injected
                } else {
                    RequestSource::Person
                };
                turns.start(Some(id.to_owned()), stamp());
                turns.request(stamp(), text, from);
            }
            (Some("message"), Some("assistant")) => {
                turns.current();
                for part in message["content"].as_array().into_iter().flatten() {
                    match part["type"].as_str() {
                        Some("text") => turns.message(id, part["text"].as_str().unwrap_or("")),
                        Some("thinking") => {
                            turns.thought(id, part["thinking"].as_str().unwrap_or(""))
                        }
                        Some("toolCall") => turns.tool(
                            part["id"].as_str(),
                            part["name"].as_str(),
                            part["arguments"].clone(),
                        ),
                        _ => {}
                    }
                }
                let usage = &message["usage"];
                let count = |key: &str| usage[key].as_u64().unwrap_or(0);
                calls.count(
                    turns.turns.len(),
                    [
                        count("input"),
                        count("cacheRead"),
                        count("cacheWrite"),
                        count("output"),
                        count("reasoning"),
                    ],
                    usage["cost"]["total"].as_f64(),
                    message["model"].as_str(),
                );
                let turn = turns.current();
                calls.write(turn, "pi");
                match message["stopReason"].as_str() {
                    Some("toolUse") => {}
                    Some(stop @ ("aborted" | "error")) => {
                        if stop == "error" {
                            turn.receipt.errors.push(message["errorMessage"].clone());
                        }
                        turns.end(TurnState::Interrupted, stamp());
                    }
                    _ => turns.end(TurnState::Ended, stamp()),
                }
            }
            (Some("message"), Some("toolResult")) => turns.resolve(
                message["toolCallId"].as_str().unwrap_or_default(),
                message["isError"].as_bool() == Some(true),
                text_of(&message["content"]),
            ),
            _ => {}
        }
    }
    turns.finish()
}

/// Entries form a tree through `parentId`; a fork leaves sibling branches in
/// one file. The active branch, the one Pi resumes, is the ancestor chain of
/// the last entry, oldest first.
fn branch(rows: &[Value]) -> Vec<&Value> {
    let by_id: HashMap<&str, &Value> = rows
        .iter()
        .filter_map(|row| Some((row["id"].as_str()?, row)))
        .collect();
    let mut branch = Vec::new();
    let mut next = rows
        .iter()
        .rev()
        .find(|row| row["type"] != "session" && row["id"].is_string());
    while let Some(entry) = next {
        if branch.len() > rows.len() {
            break; // a parent cycle; the file is malformed
        }
        branch.push(entry);
        next = entry["parentId"]
            .as_str()
            .and_then(|parent| by_id.get(parent).copied());
    }
    branch.reverse();
    branch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::types::{ConversationItem, Lifecycle};

    #[test]
    fn a_pi_session_reads_only_the_branch_it_resumes_with_its_turn_end_and_cost() {
        let entry = |id: &str, parent: Option<&str>, message: Value| {
            serde_json::json!({"type":"message","id":id,"parentId":parent,
                "timestamp":"2026-10-18T23:14:46.212Z","message":message})
        };
        let rows = [
            serde_json::json!({"type":"session","version":3,"id":"s","cwd":"/repo"}),
            serde_json::json!({"type":"model_change","id":"m","parentId":null}),
            entry(
                "sys",
                Some("m"),
                serde_json::json!({"role":"system","content":""}),
            ),
            entry(
                "u1",
                Some("sys"),
                serde_json::json!({"role":"user","content":[{"type":"text","text":"List files"}]}),
            ),
            entry(
                "a1",
                Some("u1"),
                serde_json::json!({"role":"assistant","model":"gemini-4-argon","stopReason":"toolUse",
                "usage":{"input":900,"output":20,"cacheRead":100,"cacheWrite":0,"reasoning":5,
                    "cost":{"total":0.25}},
                "content":[
                {"type":"thinking","thinking":"use ls"},
                {"type":"toolCall","id":"c1","name":"bash","arguments":{"command":"ls"}}]}),
            ),
            entry(
                "r1",
                Some("a1"),
                serde_json::json!({"role":"toolResult","toolCallId":"c1","isError":true,
                "content":[{"type":"text","text":"denied"}]}),
            ),
            entry(
                "abandoned",
                Some("u1"),
                serde_json::json!({"role":"assistant","content":[{"type":"text","text":"forked away"}]}),
            ),
            entry(
                "a2",
                Some("r1"),
                serde_json::json!({"role":"assistant","model":"gemini-4-argon","stopReason":"stop",
                "usage":{"input":50,"output":10,"cacheRead":950,"cacheWrite":0,"reasoning":0,
                    "cost":{"total":0.5}},
                "content":[{"type":"text","text":"ls was denied"}]}),
            ),
        ];
        let turns = read(&rows);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].requests().next().unwrap().text, "List files");
        assert_eq!(turns[0].id.as_deref(), Some("u1"));
        assert_eq!(turns[0].state, TurnState::Ended);
        assert_eq!(turns[0].model.as_deref(), Some("gemini-4-argon"));
        assert_eq!(
            turns[0].usage,
            Some(crate::transcript::live::TurnUsage {
                input_tokens: 2000,
                cached_input_tokens: 1050,
                output_tokens: 30,
            })
        );
        assert_eq!(turns[0].receipt.usage["usage"]["cost_usd"], 0.75);
        assert_eq!(turns[0].receipt.usage["usage"]["reasoning_tokens"], 5);
        let said = turns[0].agent().collect::<Vec<_>>();
        assert!(matches!(said[1],
            ConversationItem::Tool { status: Lifecycle::Failed, output: Some(output), .. } if output == "denied"));
        assert!(
            matches!(said[2], ConversationItem::Message { text, .. } if text == "ls was denied")
        );
        assert_eq!(said.len(), 3);
    }
}
