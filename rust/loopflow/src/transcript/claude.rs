//! Claude transcripts: `projects/<directory>/<id>.jsonl`, one row per message
//! block. Sidechains belong to subagents and are not this conversation.

use serde_json::Value;

use super::{text_of, unix_seconds, RequestSource, Turn, Turns};

pub(super) fn turns(rows: &[Value]) -> Vec<Turn> {
    let mut turns = Turns::default();
    for row in rows {
        if row["isSidechain"].as_bool() == Some(true) {
            continue;
        }
        let content = &row["message"]["content"];
        let id = row["uuid"].as_str().unwrap_or_default();
        match row["type"].as_str() {
            Some("user") => {
                let at = unix_seconds(&row["timestamp"]);
                if is_user_input(row) {
                    turns.request(at, text_of(content), RequestSource::Person);
                    continue;
                }
                let text = text_of(content);
                if row["isMeta"].as_bool() != Some(true) && !text.trim().is_empty() {
                    turns.request(at, text, RequestSource::Injected);
                }
                for block in content.as_array().into_iter().flatten() {
                    if block["type"] == "tool_result" {
                        turns.resolve(
                            block["tool_use_id"].as_str().unwrap_or_default(),
                            block["is_error"].as_bool() == Some(true),
                            text_of(&block["content"]),
                        );
                    }
                }
            }
            Some("assistant") => {
                for block in content.as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => turns.message(id, block["text"].as_str().unwrap_or("")),
                        Some("thinking") => {
                            turns.thought(id, block["thinking"].as_str().unwrap_or(""))
                        }
                        Some("tool_use") => turns.tool(
                            block["id"].as_str(),
                            block["name"].as_str(),
                            block["input"].clone(),
                        ),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    turns.0
}

/// Whether a Claude transcript row is something a person typed: not a tool
/// result, sidechain, meta line, compaction, or Loopflow/harness-injected block.
pub(crate) fn is_user_input(row: &Value) -> bool {
    if row["type"] != "user"
        || row["userType"] != "external"
        || ["isSidechain", "isMeta", "isCompactSummary", "isSynthetic"]
            .iter()
            .any(|key| row[key].as_bool() == Some(true))
        || ["sourceToolAssistantUUID", "agentId", "toolUseResult"]
            .iter()
            .any(|key| !row[key].is_null())
    {
        return false;
    }
    let content = &row["message"]["content"];
    let texts = if let Some(text) = content.as_str() {
        vec![text]
    } else if let Some(blocks) = content.as_array() {
        blocks
            .iter()
            .filter(|block| block["type"] == "text")
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
    } else {
        return false;
    };
    let image = content
        .as_array()
        .is_some_and(|blocks| blocks.iter().any(|block| block["type"] == "image"));
    (image || texts.iter().any(|text| !text.trim().is_empty()))
        && !texts.iter().any(|text| {
            [
                "<lf:",
                "<task-notification>",
                "<local-command",
                "<command-name>",
                "<system-reminder>",
                "<session-start-hook>",
                "<tick>",
                "<goal>",
                "<ide_opened_file>",
                "<ide_selection>",
                "[Request interrupted by user",
                "This session is being continued from a previous conversation",
            ]
            .iter()
            .any(|marker| text.contains(marker))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::types::{ConversationItem, Lifecycle};

    #[test]
    fn a_claude_transcript_pairs_tool_results_and_separates_injected_requests() {
        let rows = [
            serde_json::json!({"type":"user","userType":"external","timestamp":"2026-10-26T10:00:00Z","message":{"content":"<lf:message>\nFix it\n</lf:message>"}}),
            serde_json::json!({"type":"assistant","uuid":"a1","message":{"content":[
                {"type":"thinking","thinking":"plan"},
                {"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}]}}),
            serde_json::json!({"type":"user","userType":"external","toolUseResult":"ok","message":{"content":[
                {"type":"tool_result","tool_use_id":"t1","content":"Cargo.toml"}]}}),
            serde_json::json!({"type":"user","userType":"external","isSidechain":true,"message":{"content":"subagent"}}),
            serde_json::json!({"type":"user","userType":"external","timestamp":"2026-10-26T10:05:00Z","message":{"content":"Now ship"}}),
        ];
        let turns = turns(&rows);
        assert_eq!(turns.len(), 2);
        assert_eq!(
            turns[0].request.as_ref().unwrap().from,
            RequestSource::Injected
        );
        assert!(
            matches!(&turns[0].items[0], ConversationItem::Thought { text, .. } if text == "plan")
        );
        assert!(matches!(&turns[0].items[1],
            ConversationItem::Tool { status: Lifecycle::Completed, output: Some(output), .. } if output == "Cargo.toml"));
        let person = turns[1].request.as_ref().unwrap();
        assert_eq!(
            (person.text.as_str(), person.from),
            ("Now ship", RequestSource::Person)
        );
        assert_eq!(turns[1].at, Some(1_793_009_100));
    }
}
