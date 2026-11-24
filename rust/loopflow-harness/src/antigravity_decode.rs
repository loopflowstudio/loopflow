//! `agy --output-format stream-json` lines, decoded once each. One process is
//! one turn: `init` names the conversation, each `step_update` reports a step
//! of it, and `result` settles it. Usage comes from the transcript reader
//! (`transcript::antigravity`), so the stream writes none.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::fact::{Decoded, ProviderFact, ProviderTurn, Signal};
use loopflow_model::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use loopflow_model::id::AgentSessionId;

pub const ERROR_CODE: &str = "agy_error";

/// One Loopflow turn in an Antigravity conversation.
#[derive(Debug)]
pub struct Turn {
    /// The conversation, once lf resumed it or `agy` named it.
    thread: Option<AgentSessionId>,
    /// The turn's name, once a step or a failure began it.
    id: Option<String>,
    /// The agent response being written: its step and its text so far.
    writing: (i64, String),
    final_text: String,
    /// The turn's first failure, kept on its completion.
    error: Option<Value>,
    /// `agy` printed a `result` that says the turn succeeded.
    succeeded: bool,
    /// Tool steps that have started and not finished.
    open: BTreeMap<String, Value>,
}

impl Turn {
    pub fn new(thread: Option<AgentSessionId>) -> Self {
        Self {
            thread,
            id: None,
            writing: (-1, String::new()),
            final_text: String::new(),
            error: None,
            succeeded: false,
            open: BTreeMap::new(),
        }
    }

    pub fn thread(&self) -> Option<&AgentSessionId> {
        self.thread.as_ref()
    }

    /// Decode one line the run printed.
    pub fn event(&mut self, line: &Value) -> Vec<Decoded> {
        match line["event"].as_str() {
            Some("init") => self.observe(&line["conversation_id"]),
            Some("step_update") => self.step(&line["step_update"]),
            Some("result") => {
                let result = &line["result"];
                let mut out = self.observe(&result["conversation_id"]);
                if result["status"] == "SUCCESS" {
                    self.succeeded = true;
                    if let Some(response) = result["response"].as_str() {
                        if !response.trim().is_empty() {
                            self.final_text = response.trim_end().into();
                        }
                    }
                } else {
                    // A refused run exits 0 when its stdin was not a pipe: the
                    // status fails the turn, never the exit code.
                    out.extend(self.fail(result["error"].as_str().unwrap_or_default()));
                }
                out
            }
            _ => Vec::new(),
        }
    }

    fn observe(&mut self, conversation: &Value) -> Vec<Decoded> {
        match conversation.as_str().filter(|id| !id.is_empty()) {
            Some(id) if self.thread.as_ref().map(AgentSessionId::as_str) != Some(id) => {
                let thread = AgentSessionId::from(id);
                self.thread = Some(thread.clone());
                vec![ProviderFact::SessionObserved(thread).into()]
            }
            _ => Vec::new(),
        }
    }

    /// Name the turn. A turn is `step-<n>`, the index of its `user_input`
    /// step, which the transcript's `USER_INPUT` row carries too; a run that
    /// fails before any step gets a name of lf's own.
    fn begin(&mut self, input_step: Option<i64>) -> Vec<Decoded> {
        if self.id.is_some() {
            return Vec::new();
        }
        let id = input_step.map_or_else(
            || uuid::Uuid::new_v4().to_string(),
            |step| format!("step-{step}"),
        );
        self.id = Some(id.clone());
        let mut out: Vec<Decoded> = Vec::new();
        if let Some(thread) = &self.thread {
            out.push(
                ProviderFact::TurnAnswered {
                    turn: ProviderTurn {
                        thread: thread.clone(),
                        turn: id.clone(),
                    },
                    request: id.clone(),
                }
                .into(),
            );
        }
        out.push(ConversationEvent::TurnStarted { turn_id: id }.into());
        out
    }

    fn step(&mut self, step: &Value) -> Vec<Decoded> {
        let index = step["step_index"].as_i64();
        let kind = step["step_type"].as_str().unwrap_or_default();
        let mut out = self.begin(index.filter(|_| kind == "user_input"));
        out.push(ProviderFact::Activity(Signal::Working).into());
        let id = self.id.clone().expect("the turn has begun");
        let index = index.unwrap_or_default();
        let done = step["state"] == "DONE";
        if kind == "agent_response" {
            if self.writing.0 != index {
                self.writing = (index, String::new());
            }
            if let Some(delta) = step["text_delta"].as_str().filter(|text| !text.is_empty()) {
                self.writing.1.push_str(delta);
                out.push(
                    ConversationEvent::TextDelta {
                        turn_id: id.clone(),
                        content: delta.into(),
                    }
                    .into(),
                );
            }
            let text = self.writing.1.trim_end();
            if done && !text.is_empty() {
                self.final_text = text.into();
                out.push(
                    ConversationEvent::ItemCompleted {
                        turn_id: id.clone(),
                        item: ConversationItem::Message {
                            id: format!("{id}:{index}"),
                            text: text.into(),
                            phase: Some("commentary".into()),
                        },
                    }
                    .into(),
                );
            }
        } else if step.get("tool_name").is_some() {
            let tool = format!("{id}:{index}");
            let status = match step["state"].as_str() {
                Some("DONE") => Lifecycle::Completed,
                Some("ERROR" | "CANCELED" | "INTERRUPTED" | "INVALID") => Lifecycle::Failed,
                _ => Lifecycle::Running,
            };
            // A fast tool prints only its finished step.
            if !self.open.contains_key(&tool) {
                out.push(ProviderFact::Activity(Signal::ToolStarted(tool.clone())).into());
                out.push(
                    ConversationEvent::ItemStarted {
                        turn_id: id.clone(),
                        item: tool_item(step, &tool, Lifecycle::Running),
                    }
                    .into(),
                );
            }
            if status == Lifecycle::Running {
                self.open.insert(tool, step.clone());
            } else {
                self.open.remove(&tool);
                out.push(ProviderFact::Activity(Signal::ToolResolved(tool.clone())).into());
                out.push(
                    ConversationEvent::ItemCompleted {
                        turn_id: id,
                        item: tool_item(step, &tool, status),
                    }
                    .into(),
                );
            }
        }
        out
    }

    /// Fail the turn for `reason`; its first failure is the one reported, by
    /// its first line (a refused model lists every model after it).
    pub fn fail(&mut self, reason: &str) -> Vec<Decoded> {
        let mut out = self.begin(None);
        if self.error.is_some() {
            return out;
        }
        let line = reason
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("Antigravity reported an error");
        let message = crate::redact::sanitize_error_message(line.trim());
        self.error = Some(loopflow_model::chat::types::turn_error(
            ERROR_CODE, &message,
        ));
        out.push(
            ConversationEvent::Error {
                code: ERROR_CODE.into(),
                message,
                evidence: None,
            }
            .into(),
        );
        out
    }

    /// End the turn as its process ended. `interrupted` is whether lf stopped
    /// it; `stderr` is what `agy` wrote there, read when it left no `result`.
    pub fn finish(&mut self, interrupted: bool, stderr: &str) -> Vec<Decoded> {
        let mut out = self.begin(None);
        if !interrupted && !self.succeeded {
            let last = stderr.lines().rev().find(|line| !line.trim().is_empty());
            out.extend(self.fail(last.unwrap_or("Antigravity exited before finishing the turn")));
        }
        let id = self.id.clone().expect("the turn has begun");
        let status = if interrupted {
            Lifecycle::Interrupted
        } else if self.error.is_some() {
            Lifecycle::Failed
        } else {
            Lifecycle::Completed
        };
        for (tool, step) in std::mem::take(&mut self.open) {
            out.push(ProviderFact::Activity(Signal::ToolResolved(tool.clone())).into());
            out.push(
                ConversationEvent::ItemCompleted {
                    turn_id: id.clone(),
                    item: tool_item(&step, &tool, Lifecycle::Failed),
                }
                .into(),
            );
        }
        out.push(ProviderFact::Activity(Signal::Yielded(status == Lifecycle::Completed)).into());
        if !self.final_text.is_empty() {
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
                turn: self.thread.clone().map(|thread| ProviderTurn {
                    thread,
                    turn: id.clone(),
                }),
                status,
                detail: json!({"status": status.name(), "error": self.error}),
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

fn tool_item(step: &Value, id: &str, status: Lifecycle) -> ConversationItem {
    let info = &step["tool_info"];
    let output = (status != Lifecycle::Running)
        .then(|| info["output"].as_str().map(str::to_owned))
        .flatten();
    if let Some(line) = info["parameters"]["CommandLine"].as_str() {
        return ConversationItem::Command {
            id: id.into(),
            // A whole command line, not argv.
            command: vec![line.to_owned()],
            cwd: info["parameters"]["Cwd"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            status,
            output,
            exit_code: None,
            duration_ms: step["duration_seconds"]
                .as_f64()
                .map(|seconds| (seconds * 1000.0) as u64),
        };
    }
    ConversationItem::Tool {
        id: id.into(),
        name: step["tool_name"].as_str().unwrap_or("tool").into(),
        status,
        // A subagent step describes its subagents where a tool has parameters.
        input: info
            .get("parameters")
            .or_else(|| step.get("subagent_info"))
            .cloned(),
        output,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fact::{fact_lines, trace};

    fn errors(decoded: &[Decoded]) -> Vec<(&str, &str)> {
        decoded
            .iter()
            .filter_map(|decoded| match decoded {
                Decoded::Display(ConversationEvent::Error { code, message, .. }) => {
                    Some((code.as_str(), message.as_str()))
                }
                _ => None,
            })
            .collect()
    }

    /// `antigravity_run.jsonl` is what `agy` 1.5.13 printed for a prompt that
    /// ran `echo hi` and answered "done" (its list of 60 tools cut to two).
    #[test]
    fn a_recorded_run_names_its_conversation_and_turn_and_settles_with_its_answer() {
        let mut turn = Turn::new(None);
        let mut decoded: Vec<Decoded> = trace("antigravity_run.jsonl")
            .iter()
            .flat_map(|line| turn.event(line))
            .collect();
        decoded.extend(turn.finish(false, ""));

        const THREAD: &str = "ac05d4e6-42b1-42e1-8f6a-94ea3c3e0c12";
        assert_eq!(turn.thread().map(AgentSessionId::as_str), Some(THREAD));
        assert_eq!(
            fact_lines(&decoded),
            [
                format!("SessionObserved {THREAD}"),
                format!("TurnAnswered {THREAD}/step-0 request=step-0"),
                "Activity Working".into(),
                "Activity Working".into(),
                "Activity Working".into(),
                "Activity ToolStarted(\"step-0:2\")".into(),
                "Activity Working".into(),
                "Activity ToolResolved(\"step-0:2\")".into(),
                "Activity Working".into(),
                "Activity Working".into(),
                "Activity Yielded(true)".into(),
                format!("TurnCompleted {THREAD}/step-0 Completed"),
            ]
        );
        assert!(decoded.iter().any(|decoded| matches!(
            decoded,
            Decoded::Display(ConversationEvent::ItemCompleted {
                item: ConversationItem::Command { command, output: Some(output), .. },
                ..
            }) if command == &["echo hi"] && output.ends_with("hi\r\n")
        )));
        assert!(decoded.iter().any(|decoded| matches!(
            decoded,
            Decoded::Display(ConversationEvent::ItemCompleted {
                item: ConversationItem::Message { text, phase: Some(phase), .. },
                ..
            }) if text == "done" && phase == "final"
        )));
        assert!(errors(&decoded).is_empty());
    }

    /// `antigravity_unknown_model.jsonl` is the one line `agy` 1.5.13 printed,
    /// exiting 1, for `--model no-such-model-xyz`.
    #[test]
    fn a_recorded_refusal_fails_a_turn_that_has_no_conversation() {
        let mut turn = Turn::new(None);
        let mut decoded: Vec<Decoded> = trace("antigravity_unknown_model.jsonl")
            .iter()
            .flat_map(|line| turn.event(line))
            .collect();
        decoded.extend(turn.finish(false, "error: ignored, the result said why"));

        assert_eq!(turn.thread(), None);
        assert_eq!(
            errors(&decoded),
            [(
                "agy_error",
                "invalid model selection (--model \"no-such-model-xyz\" --effort \"\"): model no-such-model-xyz is not recognized as a known model or custom model in settings"
            )]
        );
        assert_eq!(
            fact_lines(&decoded),
            ["Activity Yielded(false)", "TurnCompleted - Failed"]
        );
    }

    #[test]
    fn a_result_that_says_error_fails_the_turn_whatever_the_exit_code() {
        let mut turn = Turn::new(Some("resumed".into()));
        let mut decoded = turn.event(&json!({"event":"result","result":{
            "conversation_id":"","status":"ERROR","error":"authentication failed or timed out"}}));
        decoded.extend(turn.finish(false, ""));
        assert_eq!(
            errors(&decoded),
            [("agy_error", "authentication failed or timed out")]
        );
        assert!(fact_lines(&decoded).last().unwrap().ends_with(" Failed"));
    }

    #[test]
    fn a_run_that_leaves_no_result_fails_with_its_last_stderr_line() {
        let mut turn = Turn::new(None);
        let decoded = turn.finish(false, "warming up\nError: the model stream broke\n\n");
        assert_eq!(
            errors(&decoded),
            [("agy_error", "Error: the model stream broke")]
        );

        let mut silent = Turn::new(None);
        assert_eq!(
            errors(&silent.finish(false, "")),
            [("agy_error", "Antigravity exited before finishing the turn")]
        );
    }

    #[test]
    fn a_resumed_turn_is_named_for_its_own_input_step() {
        let mut turn = Turn::new(Some("thread".into()));
        let step = |index: i64, kind: &str| {
            json!({"event":"step_update","step_update":{
                "conversation_id":"thread","step_index":index,"state":"DONE","step_type":kind}})
        };
        let mut decoded = turn.event(&json!({"event":"init","conversation_id":"thread"}));
        decoded.extend(turn.event(&step(7, "user_input")));
        decoded.extend(turn.event(&step(8, "system_message")));
        assert_eq!(
            fact_lines(&decoded),
            [
                "TurnAnswered thread/step-7 request=step-7",
                "Activity Working",
                "Activity Working"
            ]
        );
    }

    #[test]
    fn a_stopped_run_is_interrupted_and_closes_the_tool_it_left_running() {
        let mut turn = Turn::new(Some("thread".into()));
        turn.event(&json!({"event":"step_update","step_update":{
            "step_index":0,"state":"DONE","step_type":"user_input"}}));
        turn.event(&json!({"event":"step_update","step_update":{
            "step_index":2,"state":"ACTIVE","step_type":"tool","tool_name":"run_command",
            "tool_info":{"name":"run_command","parameters":{"CommandLine":"sleep 60"}}}}));
        let decoded = turn.finish(true, "");
        assert_eq!(
            fact_lines(&decoded),
            [
                "Activity ToolResolved(\"step-0:2\")",
                "Activity Yielded(false)",
                "TurnCompleted thread/step-0 Interrupted"
            ]
        );
        assert!(errors(&decoded).is_empty());
    }
}
