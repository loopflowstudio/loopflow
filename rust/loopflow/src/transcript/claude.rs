//! Claude transcripts: `projects/<directory>/<id>.jsonl`, one row per message
//! block. Sidechains belong to subagents and are not this conversation.

use serde_json::{json, Value};

use super::live::{TurnState, TurnUsage};
use super::{text_of, unix_seconds, RequestSource, Runs, Transcript, Turns};

/// A run ends with an assistant message whose `stop_reason` is `end_turn`,
/// usually followed by a `turn_duration` system row. A run's id is the `uuid`
/// of the request row that started it.
pub(super) fn read(rows: &[Value]) -> Transcript {
    let mut turns = Turns::default();
    let mut runs = Runs::default();
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
                let at = unix_seconds(&row["timestamp"]);
                if is_user_input(row) {
                    turns.request(at, text_of(content), RequestSource::Person);
                    tally.asked.clear();
                    runs.start(request(), stamp());
                    continue;
                }
                let text = text_of(content);
                if row["isMeta"].as_bool() != Some(true) && !text.trim().is_empty() {
                    turns.request(at, text, RequestSource::Injected);
                }
                let mut answered = false;
                for block in content.as_array().into_iter().flatten() {
                    if block["type"] == "tool_result" {
                        answered = true;
                        let call = block["tool_use_id"].as_str().unwrap_or_default();
                        tally.asked.retain(|asked| asked != call);
                        turns.resolve(
                            call,
                            block["is_error"].as_bool() == Some(true),
                            text_of(&block["content"]),
                        );
                    }
                }
                if is_interruption(row) {
                    tally.asked.clear();
                    runs.end(TurnState::Interrupted, stamp());
                } else if answered {
                    runs.current().state = tally.state();
                } else {
                    runs.wake(request(), stamp());
                }
            }
            Some("assistant") => {
                let message = &row["message"];
                for block in content.as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => turns.message(id, block["text"].as_str().unwrap_or("")),
                        Some("thinking") => {
                            turns.thought(id, block["thinking"].as_str().unwrap_or(""))
                        }
                        Some("tool_use") => {
                            if block["name"] == QUESTION {
                                tally.asked.extend(block["id"].as_str().map(str::to_owned));
                            }
                            turns.tool(
                                block["id"].as_str(),
                                block["name"].as_str(),
                                block["input"].clone(),
                            )
                        }
                        _ => {}
                    }
                }
                runs.current();
                let at = runs.runs.len();
                let run = runs.runs.last_mut().expect("the current run");
                tally.count(at, message);
                (run.usage, run.receipt) = tally.usage();
                run.model = tally.model.clone();
                if message["stop_reason"] == "end_turn" {
                    tally.asked.clear();
                    run.state = TurnState::Ended;
                    run.ended_at = stamp();
                } else {
                    run.state = tally.state();
                    run.ended_at = None;
                }
            }
            Some("system") => match (row["subtype"].as_str(), runs.runs.last_mut()) {
                (Some("turn_duration"), Some(run)) if run.state == TurnState::Ended => {
                    run.duration_ms = row["durationMs"].as_u64();
                }
                (Some("api_error"), Some(run)) if run.ended_at.is_none() => {
                    run.errors.push(row["error"].clone());
                }
                _ => {}
            },
            _ => {}
        }
    }
    Transcript {
        turns: turns.0,
        runs: runs.runs,
    }
}

/// The tool Claude asks a person a question with. Its call stays without a
/// result until they answer.
const QUESTION: &str = "AskUserQuestion";

/// One run's model calls and open questions.
#[derive(Debug, Default)]
struct Tally {
    /// Which run the calls belong to, by its position.
    run: usize,
    /// Usage by message id: rows repeat one message for each content block.
    calls: Vec<(String, [u64; 4])>,
    model: Option<String>,
    asked: Vec<String>,
}

impl Tally {
    fn count(&mut self, run: usize, message: &Value) {
        if self.run != run {
            *self = Self {
                run,
                asked: std::mem::take(&mut self.asked),
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

    fn state(&self) -> TurnState {
        if self.asked.is_empty() {
            TurnState::Running
        } else {
            TurnState::Asking
        }
    }

    /// The run's usage so far, and the same as the live stream's receipt.
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
            json!({"provider": "claude", "usage": {
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
        let turns = read(&rows).turns;
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

    fn user(uuid: &str, at: &str, content: Value) -> Value {
        serde_json::json!({"type":"user","userType":"external","uuid":uuid,
            "timestamp":format!("2026-11-10T16:{at}Z"),"message":{"content":content}})
    }

    fn assistant(message: &str, at: &str, stop: &str, content: Value) -> Value {
        serde_json::json!({"type":"assistant","timestamp":format!("2026-11-10T16:{at}Z"),
            "message":{"id":message,"model":"claude-opus-5-5","stop_reason":stop,"content":content,
                "usage":{"input_tokens":1,"output_tokens":10}}})
    }

    #[test]
    fn an_unanswered_question_reads_as_asking_until_its_answer_arrives() {
        let question =
            serde_json::json!([{"type":"tool_use","id":"q1","name":"AskUserQuestion","input":{}}]);
        let mut rows = vec![
            user("u1", "00:00", "Pick a color".into()),
            assistant("m1", "00:05", "tool_use", question),
        ];
        assert_eq!(read(&rows).runs[0].state, TurnState::Asking);

        let mut answer = user(
            "u2",
            "03:00",
            serde_json::json!([{"type":"tool_result","tool_use_id":"q1","content":"blue"}]),
        );
        answer["toolUseResult"] = "blue".into();
        rows.push(answer);
        let runs = read(&rows).runs;
        assert_eq!((runs.len(), runs[0].state), (1, TurnState::Running));
    }

    #[test]
    fn a_run_starts_at_what_woke_the_agent_and_holds_input_queued_before_it_ends() {
        let text = |text: &str| serde_json::json!([{"type":"text","text":text}]);
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
        let runs = read(&rows).runs;
        // The launch message woke the agent; typing over it ended that run.
        assert_eq!(
            runs.iter()
                .map(|run| (run.id.as_deref(), run.state, run.usage.output_tokens))
                .collect::<Vec<_>>(),
            [
                (Some("u1"), TurnState::Interrupted, 10),
                (Some("u2"), TurnState::Ended, 10),
            ]
        );
        assert_eq!(runs[0].ended_at.as_deref(), Some("2026-11-10T16:00:30Z"));

        // Rows no model call follows start nothing; the notification's does.
        let mut rows = rows.to_vec();
        rows.push(assistant("m3", "05:02", "tool_use", text("looking")));
        let runs = read(&rows).runs;
        assert_eq!(
            (runs.len(), runs[2].id.as_deref(), runs[2].state),
            (3, Some("n1"), TurnState::Running)
        );
        assert_eq!(runs[2].started_at.as_deref(), Some("2026-11-10T16:05:00Z"));
    }
}
