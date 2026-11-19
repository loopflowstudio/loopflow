//! `opencode run --format json` lines, decoded once each. One process is one
//! turn: it prints each finished part and exits, and its exit settles the
//! turn. Usage comes from the transcript reader (`transcript::opencode`), so
//! the stream writes none.

use serde_json::{json, Value};

use super::fact::{Decoded, ProviderFact, ProviderTurn, Signal};
use loopflow_model::chat::types::{ConversationEvent, ConversationItem, FileEdit, Lifecycle};

/// One Loopflow turn in an OpenCode conversation.
#[derive(Debug)]
pub(super) struct Turn {
    turn: ProviderTurn,
    final_text: String,
    failed: bool,
    /// The model asked a question; its run is stopped, not left to assume an answer.
    asked: bool,
    parts: usize,
}

impl Turn {
    pub fn new(turn: ProviderTurn) -> Self {
        Self {
            turn,
            final_text: String::new(),
            failed: false,
            asked: false,
            parts: 0,
        }
    }

    /// Decode one line the run printed.
    pub fn event(&mut self, line: &Value) -> Vec<Decoded> {
        let id = self.turn.turn.clone();
        let part = &line["part"];
        let mut out: Vec<Decoded> = Vec::new();
        match line["type"].as_str() {
            Some("step_start") => out.push(ProviderFact::Activity(Signal::Working).into()),
            Some("text") => {
                let text = part["text"].as_str().unwrap_or_default();
                if text.is_empty() {
                    return out;
                }
                self.parts += 1;
                self.final_text = text.into();
                out.push(ProviderFact::Activity(Signal::Working).into());
                out.push(
                    ConversationEvent::TextDelta {
                        turn_id: id.clone(),
                        content: text.into(),
                    }
                    .into(),
                );
                out.push(
                    ConversationEvent::ItemCompleted {
                        turn_id: id.clone(),
                        item: ConversationItem::Message {
                            id: format!("{id}:{}", self.parts),
                            text: text.into(),
                            phase: Some("commentary".into()),
                        },
                    }
                    .into(),
                );
            }
            Some("reasoning") => {
                if let Some(text) = part["text"].as_str().filter(|text| !text.is_empty()) {
                    self.parts += 1;
                    out.push(
                        ConversationEvent::ItemCompleted {
                            turn_id: id.clone(),
                            item: ConversationItem::Thought {
                                id: format!("{id}:{}", self.parts),
                                text: text.into(),
                            },
                        }
                        .into(),
                    );
                }
            }
            // A run prints a tool part once, when the tool has finished.
            Some("tool_use") => {
                let Some(tool) = part["id"].as_str() else {
                    return out;
                };
                let status = match part["state"]["status"].as_str() {
                    Some("completed") => Lifecycle::Completed,
                    _ => Lifecycle::Failed,
                };
                out.push(ProviderFact::Activity(Signal::ToolStarted(tool.into())).into());
                out.push(
                    ConversationEvent::ItemStarted {
                        turn_id: id.clone(),
                        item: tool_item(part, tool, Lifecycle::Running),
                    }
                    .into(),
                );
                out.push(ProviderFact::Activity(Signal::ToolResolved(tool.into())).into());
                out.push(
                    ConversationEvent::ItemCompleted {
                        turn_id: id.clone(),
                        item: tool_item(part, tool, status),
                    }
                    .into(),
                );
                // `run` cancels a question itself and tells the model to go on
                // with assumptions; a worker's question ends its turn instead.
                if part["tool"] == "question" && status != Lifecycle::Completed {
                    let asked: Vec<&str> = part["state"]["input"]["questions"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|question| question["question"].as_str())
                        .collect();
                    self.asked = true;
                    out.extend(self.fail(&super::common::unanswered_question(
                        "OpenCode",
                        &asked.join(" "),
                    )));
                }
            }
            Some("error") => {
                let error = &line["error"];
                let message = error["data"]["message"]
                    .as_str()
                    .or_else(|| error["message"].as_str())
                    .unwrap_or("OpenCode reported an error");
                out.extend(self.fail(message));
            }
            _ => {}
        }
        out
    }

    pub fn asked(&self) -> bool {
        self.asked
    }

    /// Fail the turn for `reason`; its first failure is the one reported.
    pub fn fail(&mut self, reason: &str) -> Vec<Decoded> {
        if std::mem::replace(&mut self.failed, true) {
            return Vec::new();
        }
        vec![ConversationEvent::Error {
            code: "opencode_error".into(),
            message: sanitize_error_message(reason),
            evidence: None,
        }
        .into()]
    }

    /// End the turn as its process ended: `exited` is whether it left with
    /// status zero, `interrupted` whether lf stopped it.
    pub fn finish(&mut self, exited: bool, interrupted: bool) -> Vec<Decoded> {
        let id = self.turn.turn.clone();
        let mut out: Vec<Decoded> = Vec::new();
        let answered = !self.final_text.trim().is_empty();
        let status = if interrupted {
            Lifecycle::Interrupted
        } else if self.failed || !exited || !answered {
            Lifecycle::Failed
        } else {
            Lifecycle::Completed
        };
        if status == Lifecycle::Failed {
            out.extend(self.fail(if exited {
                "OpenCode finished without a final answer"
            } else {
                "OpenCode exited before finishing the turn"
            }));
        }
        out.push(ProviderFact::Activity(Signal::Yielded(status == Lifecycle::Completed)).into());
        if answered {
            out.push(
                ConversationEvent::ItemCompleted {
                    turn_id: id.clone(),
                    item: ConversationItem::Message {
                        id: format!("{id}:final"),
                        text: self.final_text.clone(),
                        phase: Some("final".into()),
                    },
                }
                .into(),
            );
        }
        out.push(
            ProviderFact::TurnCompleted {
                turn: Some(self.turn.clone()),
                status,
                detail: json!({"status": status.name()}),
            }
            .into(),
        );
        out.push(
            ConversationEvent::TurnCompleted {
                turn_id: id,
                status,
            }
            .into(),
        );
        out
    }
}

/// Redact credential material from an error OpenCode printed before it is
/// recorded: `Authorization` header values, `Bearer` tokens and common token
/// query parameters an upstream error can carry.
fn sanitize_error_message(message: &str) -> String {
    use regex::Regex;
    let mut out = message.to_string();
    // Bearer first so `Authorization: Bearer <token>` is caught by both the
    // bearer pattern and the authorization-header pattern. The bearer pattern
    // stops at whitespace or a comma so the trailing comma that delimits the
    // authorization-header value survives as a stop boundary for the next
    // pattern — otherwise `Bearer <tok>, token=abc123` lets the authorization
    // regex eat through `token=abc123` before the param redaction runs.
    let redactions: [(Regex, &str); 3] = [
        (
            Regex::new(r"(?i)bearer\s+[^,\s]+").expect("valid credential pattern"),
            "bearer [redacted]",
        ),
        (
            Regex::new(r"(?i)authorization:\s*[^\n,]+").expect("valid credential pattern"),
            "authorization: [redacted]",
        ),
        (
            Regex::new(r"(?i)(token|key|access_token|api_key)=\S+")
                .expect("valid credential pattern"),
            "$1=[redacted]",
        ),
    ];
    for (re, replacement) in &redactions {
        out = re.replace_all(&out, *replacement).to_string();
    }
    out
}

fn tool_item(part: &Value, id: &str, status: Lifecycle) -> ConversationItem {
    let state = &part["state"];
    let input = &state["input"];
    let output = (status != Lifecycle::Running)
        .then(|| {
            state["output"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| state.get("error").map(Value::to_string))
        })
        .flatten();
    if let Some(command) = input.get("command") {
        return ConversationItem::Command {
            id: id.into(),
            // A string command is a whole command line, not argv.
            command: match command {
                Value::Array(argv) => argv
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                Value::String(line) if !line.is_empty() => vec![line.clone()],
                _ => Vec::new(),
            },
            cwd: input["cwd"].as_str().unwrap_or_default().to_owned(),
            status,
            output,
            exit_code: state["metadata"]["metadata"]["exit"]
                .as_i64()
                .or_else(|| state["metadata"]["exit"].as_i64())
                .map(|code| code as i32),
            duration_ms: state["time"]["end"]
                .as_u64()
                .zip(state["time"]["start"].as_u64())
                .and_then(|(end, start)| end.checked_sub(start)),
        };
    }
    if let Some(path) = input["path"]
        .as_str()
        .or_else(|| input["filePath"].as_str())
    {
        return ConversationItem::File {
            id: id.into(),
            changes: vec![FileEdit {
                path: path.into(),
                kind: None,
                diff: None,
            }],
            status,
        };
    }
    ConversationItem::Tool {
        id: id.into(),
        name: part["tool"].as_str().unwrap_or("tool").into(),
        status,
        input: state.get("input").cloned(),
        output,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::fact::{fact_lines, trace};

    fn turn() -> Turn {
        Turn::new(ProviderTurn {
            thread: "ses_trace".into(),
            turn: "turn".into(),
        })
    }

    fn display(decoded: &[Decoded]) -> Vec<&ConversationEvent> {
        decoded
            .iter()
            .filter_map(|decoded| match decoded {
                Decoded::Display(event) => Some(event),
                Decoded::Fact(_) => None,
            })
            .collect()
    }

    /// `opencode_run.jsonl` is what OpenCode 2.2.1 printed for a prompt that
    /// ran one shell command and answered "done".
    #[test]
    fn a_recorded_run_shows_its_tool_and_settles_with_its_answer() {
        let mut turn = turn();
        let mut decoded: Vec<Decoded> = trace("opencode_run.jsonl")
            .iter()
            .flat_map(|line| turn.event(line))
            .collect();
        decoded.extend(turn.finish(true, false));

        assert_eq!(
            fact_lines(&decoded),
            [
                "Activity Working",
                "Activity ToolStarted(\"call_16989002580149611345\")",
                "Activity ToolResolved(\"call_16989002580149611345\")",
                "Activity Working",
                "Activity Working",
                "Activity Yielded(true)",
                "TurnCompleted ses_trace/turn Completed",
            ]
        );
        let shown = display(&decoded);
        assert!(shown.iter().any(|event| matches!(
            event,
            ConversationEvent::ItemCompleted {
                item: ConversationItem::Command { command, output: Some(output), exit_code: Some(0), .. },
                ..
            } if command == &["echo probe"] && output == "probe\n"
        )));
        assert!(shown.iter().any(|event| matches!(
            event,
            ConversationEvent::ItemCompleted {
                item: ConversationItem::Message { text, phase: Some(phase), .. },
                ..
            } if text == "done" && phase == "final"
        )));
    }

    /// `opencode_run_question.jsonl` is what OpenCode 2.2.1 printed, exiting 0,
    /// for a prompt that told the model to ask through its question tool.
    #[test]
    fn a_recorded_question_fails_the_turn_with_what_was_asked() {
        let mut turn = turn();
        let mut decoded: Vec<Decoded> = trace("opencode_run_question.jsonl")
            .iter()
            .flat_map(|line| turn.event(line))
            .collect();
        assert!(turn.asked());
        decoded.extend(turn.finish(true, false));

        let errors: Vec<&str> = display(&decoded)
            .into_iter()
            .filter_map(|event| match event {
                ConversationEvent::Error { message, .. } => Some(message.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            errors,
            ["OpenCode asked a question with nobody there to answer it: Do you prefer red or blue?"]
        );
        assert_eq!(
            fact_lines(&decoded).last().unwrap(),
            "TurnCompleted ses_trace/turn Failed"
        );
    }

    #[test]
    fn an_error_line_or_a_bad_exit_or_no_answer_fails_the_turn() {
        let mut refused = turn();
        let decoded = refused.event(&json!({"type":"error","error":{
            "type":"unknown","message":"Expected a string starting with \"ses\""}}));
        assert!(matches!(
            display(&decoded)[..],
            [ConversationEvent::Error { code, message, .. }]
                if code == "opencode_error" && message.contains("starting with")
        ));
        assert_eq!(
            fact_lines(&refused.finish(false, false)).last().unwrap(),
            "TurnCompleted ses_trace/turn Failed"
        );

        for exited in [true, false] {
            let mut silent = turn();
            let decoded = silent.finish(exited, false);
            assert!(matches!(
                display(&decoded)[0],
                ConversationEvent::Error { code, .. } if code == "opencode_error"
            ));
            assert_eq!(
                fact_lines(&decoded).last().unwrap(),
                "TurnCompleted ses_trace/turn Failed"
            );
        }
    }

    #[test]
    fn a_stopped_run_is_interrupted_whatever_it_printed() {
        let mut turn = turn();
        turn.event(&json!({"type":"text","part":{"text":"partial"}}));
        assert_eq!(
            fact_lines(&turn.finish(false, true)).last().unwrap(),
            "TurnCompleted ses_trace/turn Interrupted"
        );
    }

    #[test]
    fn sanitize_error_message_redacts_credentials() {
        let input = "request to https://api.example.com/v1/chat?api_key=sk-secret123 failed: \
                     Authorization: Bearer super-secret-token, token=abc123";
        let sanitized = sanitize_error_message(input);
        assert!(
            !sanitized.contains("sk-secret123"),
            "api_key leaked: {sanitized}"
        );
        assert!(
            !sanitized.contains("super-secret-token"),
            "bearer token leaked: {sanitized}"
        );
        assert!(
            !sanitized.contains("abc123"),
            "token param leaked: {sanitized}"
        );
        assert!(sanitized.contains("api_key=[redacted]"));
        assert!(sanitized.contains("authorization: [redacted]"));
        assert!(sanitized.contains("token=[redacted]"));
    }

    #[test]
    fn sanitize_error_message_preserves_non_credential_text() {
        let input = "chunk stream ended (reqwest: EOF)";
        let sanitized = sanitize_error_message(input);
        assert_eq!(sanitized, input);
    }
}
