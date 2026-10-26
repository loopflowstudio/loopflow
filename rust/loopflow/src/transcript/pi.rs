//! Pi session files. Entries form a tree through `parentId`; a fork leaves
//! sibling branches in one file. The active branch, the one Pi resumes, is
//! the ancestor chain of the last entry.

use std::collections::HashMap;

use serde_json::Value;

use super::{text_of, unix_seconds, Launch, RequestSource, Turn, Turns};

pub(super) fn turns(rows: &[Value]) -> Vec<Turn> {
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
    let mut turns = Turns::default();
    for entry in branch.into_iter().rev() {
        if entry["type"] != "message" {
            continue;
        }
        let message = &entry["message"];
        let id = entry["id"].as_str().unwrap_or_default();
        match message["role"].as_str() {
            Some("user") => {
                let text = text_of(&message["content"]);
                let from = if Launch::parse(&text).is_some() || text.contains("<lf:") {
                    RequestSource::Injected
                } else {
                    RequestSource::Person
                };
                turns.request(unix_seconds(&entry["timestamp"]), text, from);
            }
            Some("assistant") => {
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
            }
            Some("toolResult") => turns.resolve(
                message["toolCallId"].as_str().unwrap_or_default(),
                message["isError"].as_bool() == Some(true),
                text_of(&message["content"]),
            ),
            _ => {}
        }
    }
    turns.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::types::{ConversationItem, Lifecycle};

    #[test]
    fn a_pi_session_reads_only_the_branch_it_resumes() {
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
                serde_json::json!({"role":"assistant","content":[
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
                serde_json::json!({"role":"assistant","content":[{"type":"text","text":"ls was denied"}]}),
            ),
        ];
        let turns = turns(&rows);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].request.as_ref().unwrap().text, "List files");
        assert!(matches!(&turns[0].items[1],
            ConversationItem::Tool { status: Lifecycle::Failed, output: Some(output), .. } if output == "denied"));
        assert!(
            matches!(&turns[0].items[2], ConversationItem::Message { text, .. } if text == "ls was denied")
        );
        assert_eq!(turns[0].items.len(), 3);
    }
}
