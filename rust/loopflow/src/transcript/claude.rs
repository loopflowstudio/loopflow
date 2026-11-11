//! Claude transcripts: `projects/<directory>/<id>.jsonl`, one row per message
//! block. Sidechains belong to subagents and are not this conversation.

use serde_json::{json, Value};

use super::live::{TurnState, TurnUsage};
use super::{text_of, RequestSource, Turn, Turns};

/// A turn ends with an assistant message whose `stop_reason` is `end_turn`,
/// usually followed by a `turn_duration` system row. A turn's id is the `uuid`
/// of the row that started it. A question the agent asked, or a tool waiting
/// on a permission prompt, reads as running: Claude writes the call's row
/// only with its result.
pub(super) fn read(rows: &[Value]) -> Vec<Turn> {
    let mut turns = Turns::default();
    let mut tally = Tally::default();
    for row in rows {
        if row["isSidechain"].as_bool() == Some(true) {
            continue;
        }
        let content = &row["message"]["content"];
        let id = row["uuid"].as_str().unwrap_or_default();
        let request = || row["uuid"].as_str().map(str::to_owned);
        let stamp = || row["timestamp"].as_str().map(str::to_owned);
        match row["type"].as_str() {
            Some("user") => {
                if is_user_input(row) {
                    turns.start(request(), stamp());
                    turns.request(stamp(), text_of(content), RequestSource::Person);
                    continue;
                }
                let text = text_of(content);
                if row["isMeta"].as_bool() != Some(true) && !text.trim().is_empty() {
                    turns.request(stamp(), text, RequestSource::Injected);
                }
                let mut answered = false;
                for block in content.as_array().into_iter().flatten() {
                    if block["type"] == "tool_result" {
                        answered = true;
                        turns.resolve(
                            block["tool_use_id"].as_str().unwrap_or_default(),
                            block["is_error"].as_bool() == Some(true),
                            text_of(&block["content"]),
                        );
                    }
                }
                if is_interruption(row) {
                    turns.end(TurnState::Interrupted, stamp());
                } else if answered {
                    turns.current();
                } else {
                    turns.wake(request(), stamp());
                }
            }
            Some("assistant") => {
                let message = &row["message"];
                turns.current();
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
                tally.count(turns.turns.len(), message);
                let turn = turns.current();
                let (usage, receipt) = tally.usage();
                turn.usage = Some(usage);
                turn.receipt.usage = receipt;
                turn.model = tally.model.clone();
                if message["stop_reason"] == "end_turn" {
                    turn.state = Some(TurnState::Ended);
                    turn.ended_at = stamp();
                }
            }
            Some("system") => match (row["subtype"].as_str(), turns.turns.last_mut()) {
                (Some("turn_duration"), Some(turn)) if turn.state == Some(TurnState::Ended) => {
                    turn.receipt.duration_ms = row["durationMs"].as_u64();
                }
                (Some("api_error"), Some(turn)) if turn.ended_at.is_none() => {
                    turn.receipt.errors.push(row["error"].clone());
                }
                _ => {}
            },
            _ => {}
        }
    }
    turns.finish()
}

/// One turn's model calls.
#[derive(Debug, Default)]
struct Tally {
    /// Which turn the calls belong to, by its position.
    turn: usize,
    /// Usage by message id: rows repeat one message for each content block.
    calls: Vec<(String, [u64; 4])>,
    model: Option<String>,
}

impl Tally {
    fn count(&mut self, turn: usize, message: &Value) {
        if self.turn != turn {
            *self = Self {
                turn,
                ..Self::default()
            };
        }
        let id = message["id"].as_str().unwrap_or_default();
        let reported = &message["usage"];
        let tokens = [
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
            "output_tokens",
        ]
        .map(|key| reported[key].as_u64().unwrap_or(0));
        match self.calls.iter_mut().find(|(call, _)| call == id) {
            Some((_, counted)) => *counted = tokens,
            None => self.calls.push((id.to_owned(), tokens)),
        }
        if let Some(model) = message["model"].as_str().filter(|m| *m != "<synthetic>") {
            self.model = Some(model.to_owned());
        }
    }

    /// The turn's usage so far, and the same as its store row.
    fn usage(&self) -> (TurnUsage, Value) {
        let [input, cache_read, cache_write, output] =
            self.calls.iter().fold([0; 4], |sum, (_, call)| {
                [0, 1, 2, 3].map(|i| sum[i] + call[i])
            });
        let total = input + cache_read + cache_write;
        (
            TurnUsage {
                input_tokens: total,
                cached_input_tokens: cache_read,
                output_tokens: output,
            },
            json!({"provider": "claude", "final": true, "usage": {
                "input_tokens": input,
                "output_tokens": output,
                "total_input_tokens": total,
                "cache_read_tokens": cache_read,
                "cache_write_tokens": cache_write,
                "model": self.model,
            }}),
        )
    }
}

fn is_interruption(row: &Value) -> bool {
    let content = &row["message"]["content"];
    let text = |text: &str| text.starts_with("[Request interrupted by user");
    content.as_str().is_some_and(text)
        || content.as_array().is_some_and(|blocks| {
            blocks
                .iter()
                .any(|block| block["text"].as_str().is_some_and(text))
        })
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
    use crate::transcript::TurnItem;

    fn user(uuid: &str, at: &str, content: Value) -> Value {
        serde_json::json!({"type":"user","userType":"external","uuid":uuid,
            "timestamp":format!("2026-11-10T16:{at}Z"),"message":{"content":content}})
    }

    fn assistant(message: &str, at: &str, stop: &str, content: Value) -> Value {
        serde_json::json!({"type":"assistant","uuid":message,
            "timestamp":format!("2026-11-10T16:{at}Z"),
            "message":{"id":message,"model":"claude-opus-5-5","stop_reason":stop,"content":content,
                "usage":{"input_tokens":1,"output_tokens":10}}})
    }

    fn text(text: &str) -> Value {
        serde_json::json!([{"type":"text","text":text}])
    }

    #[test]
    fn a_turn_holds_its_requests_and_pairs_tool_results() {
        let mut result = user(
            "r1",
            "00:07",
            serde_json::json!([{"type":"tool_result","tool_use_id":"t1","content":"Cargo.toml"}]),
        );
        result["toolUseResult"] = "ok".into();
        let mut side = user("s1", "00:08", "subagent".into());
        side["isSidechain"] = true.into();
        let rows = [
            user("u1", "00:00", "<lf:message>\nFix it\n</lf:message>".into()),
            assistant(
                "a1",
                "00:05",
                "tool_use",
                serde_json::json!([
                    {"type":"thinking","thinking":"plan"},
                    {"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}]),
            ),
            result,
            side,
            assistant("a2", "00:09", "end_turn", text("Fixed.")),
            user("u2", "05:00", "Now ship".into()),
        ];
        let turns = read(&rows);
        assert_eq!(turns.len(), 2);
        // The launch message woke the agent and is the turn's first item.
        assert_eq!(turns[0].id.as_deref(), Some("u1"));
        assert_eq!(turns[0].state, Some(TurnState::Ended));
        let TurnItem::Request(launch) = &turns[0].items[0] else {
            panic!("{:?}", turns[0].items[0]);
        };
        assert_eq!(launch.from, RequestSource::Injected);
        let said = turns[0].agent().collect::<Vec<_>>();
        assert!(matches!(said[0], ConversationItem::Thought { text, .. } if text == "plan"));
        assert!(matches!(said[1],
            ConversationItem::Tool { status: Lifecycle::Completed, output: Some(output), .. } if output == "Cargo.toml"));
        assert_eq!(said.len(), 3);

        let person = turns[1].requests().next().unwrap();
        assert_eq!(
            (person.text.as_str(), person.from),
            ("Now ship", RequestSource::Person)
        );
        assert_eq!(turns[1].at(), Some(1_794_326_700));
        assert_eq!(turns[1].state, Some(TurnState::Running));
    }

    #[test]
    fn a_question_the_agent_asked_reads_as_running_until_the_turn_ends() {
        let question =
            serde_json::json!([{"type":"tool_use","id":"q1","name":"AskUserQuestion","input":{}}]);
        let rows = [
            user("u1", "00:00", "Pick a color".into()),
            assistant("m1", "00:05", "tool_use", question),
        ];
        assert_eq!(read(&rows)[0].state, Some(TurnState::Running));
    }

    #[test]
    fn a_turn_starts_at_what_woke_the_agent_and_holds_input_queued_before_it_ends() {
        let mut caveat = user("c1", "02:00", "<local-command-caveat>ran /model".into());
        caveat["isMeta"] = true.into();
        let rows = [
            user("u1", "00:00", "<lf:message>\nFix it\n</lf:message>".into()),
            assistant("m1", "00:05", "tool_use", text("working")),
            user("u2", "00:30", "also the tests".into()),
            assistant("m2", "01:00", "end_turn", text("done")),
            caveat,
            user("n1", "05:00", "<task-notification>build finished".into()),
        ];
        let turns = read(&rows);
        // The launch message woke the agent; typing over it ended that turn.
        assert_eq!(
            turns
                .iter()
                .map(|turn| (
                    turn.id.as_deref(),
                    turn.state.unwrap(),
                    turn.usage.unwrap().output_tokens
                ))
                .collect::<Vec<_>>(),
            [
                (Some("u1"), TurnState::Interrupted, 10),
                (Some("u2"), TurnState::Ended, 10),
            ]
        );
        assert_eq!(turns[0].ended_at.as_deref(), Some("2026-11-10T16:00:30Z"));
        // A notification no model call follows starts nothing: it stays with
        // the turn before it.
        assert_eq!(turns[1].requests().count(), 2);

        // Once the agent answers it, it is the first item of a turn of its own.
        let mut rows = rows.to_vec();
        rows.push(assistant("m3", "05:02", "tool_use", text("looking")));
        let turns = read(&rows);
        assert_eq!(
            (turns.len(), turns[2].id.as_deref(), turns[2].state),
            (3, Some("n1"), Some(TurnState::Running))
        );
        assert_eq!(turns[2].started_at.as_deref(), Some("2026-11-10T16:05:00Z"));
        assert_eq!(turns[1].requests().count(), 1);
        assert!(turns[2]
            .requests()
            .next()
            .unwrap()
            .text
            .contains("build finished"));
    }
}
