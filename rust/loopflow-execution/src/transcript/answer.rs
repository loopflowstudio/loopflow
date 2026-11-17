//! A turn's final answer, read from the transcript its provider keeps.
//! Loopflow keeps no copy. A transcript that is gone, unreadable or without
//! the turn is an error naming the file, never an empty answer.

use anyhow::{anyhow, Context, Result};
use loopflow_model::chat::types::ConversationItem;
use loopflow_storage::sqlite::SqliteStore;

use super::{json_lines, locate, read, Gap, Located, Source, Turn};
use crate::harness::HarnessKind;

/// The final answer of the turn that last answered `input`; `None` when no
/// turn of it completed, or the turn ended without saying anything.
///
/// # Errors
/// When the input's provider transcript cannot be found or read, or holds no
/// such turn.
pub fn input_final_answer(store: &SqliteStore, input: &str) -> Result<Option<String>> {
    let Some(answered) = store.input_answered_turn(input)? else {
        return Ok(None);
    };
    let harness = answered
        .provider
        .as_deref()
        .and_then(HarnessKind::parse)
        .ok_or_else(|| anyhow!("input {input} names no provider whose transcript lf reads"))?;
    let source = match locate(store, harness, &answered.thread, &answered.cwd)? {
        Located::Found { source, .. } => source,
        Located::Gone(gap) => {
            let gone = match gap {
                Gap::AccountHomeRemoved => "its account home was removed".to_owned(),
                Gap::FileMissing { home } => format!("{} has no file for it", home.display()),
            };
            let error = anyhow!(
                "the transcript of conversation {} that answered {input} is gone: {gone}",
                answered.thread
            );
            tracing::warn!(%error, "final answer unavailable");
            return Err(error);
        }
    };
    last_assistant_message(harness, &source, &answered.turn)
        .with_context(|| {
            format!(
                "read the final answer of {input} from {}",
                source.path().display()
            )
        })
        .inspect_err(|error| {
            tracing::warn!(error = format!("{error:#}"), "final answer unavailable")
        })
}

/// What the agent said last in turn `turn` of a transcript, one reader per
/// provider: Codex's own record of the turn's last message, Claude's
/// structured answer or final message, and the final message of the others.
/// `None` when the turn ended without one.
///
/// # Errors
/// When the transcript cannot be read or holds no such turn.
pub fn last_assistant_message(
    harness: HarnessKind,
    source: &Source,
    turn: &str,
) -> Result<Option<String>> {
    if harness == HarnessKind::Codex {
        let completed = json_lines(source.path()).find(|row| {
            row["type"] == "event_msg"
                && row["payload"]["type"] == "task_complete"
                && row["payload"]["turn_id"] == turn
        });
        let completed = completed.ok_or_else(|| anyhow!("it has no completed turn {turn}"))?;
        return Ok(completed["payload"]["last_agent_message"]
            .as_str()
            .map(str::to_owned));
    }
    let turns = read(harness, source)?;
    let found = turns
        .iter()
        .rev()
        .find(|candidate| candidate.id.as_deref() == Some(turn))
        // Pi names its turns differently from what lf records.
        .or_else(|| turns.last().filter(|_| harness == HarnessKind::Pi))
        .ok_or_else(|| anyhow!("it has no turn {turn}"))?;
    Ok(final_message(found))
}

/// The agent's last message in `turn`, or the object a structured answer
/// returned, as JSON: Claude keeps a structured answer as its
/// `StructuredOutput` tool call.
fn final_message(turn: &Turn) -> Option<String> {
    let items: Vec<&ConversationItem> = turn.agent().collect();
    items.into_iter().rev().find_map(|item| match item {
        ConversationItem::Tool {
            name,
            input: Some(input),
            ..
        } if name == "StructuredOutput" => Some(input.to_string()),
        ConversationItem::Message { text, phase, .. }
            if phase.as_deref().is_none_or(|phase| phase == "final_answer")
                && !text.trim().is_empty() =>
        {
            Some(text.clone())
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{input_final_answer, last_assistant_message};
    use crate::harness::HarnessKind;
    use crate::transcript::Source;

    fn testdata(name: &str) -> Source {
        Source::File(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/transcript/testdata")
                .join(name),
        )
    }

    fn transcript(dir: &Path, rows: &[serde_json::Value]) -> Source {
        let path: PathBuf = dir.join("conversation.jsonl");
        let lines: Vec<String> = rows.iter().map(ToString::to_string).collect();
        std::fs::write(&path, lines.join("\n")).unwrap();
        Source::File(path)
    }

    #[test]
    fn codex_answers_with_the_last_message_it_recorded_for_the_turn() {
        let rollout = testdata("codex_rollout.jsonl");
        assert_eq!(
            last_assistant_message(HarnessKind::Codex, &rollout, "turn-2").unwrap(),
            Some("Done again.".into())
        );
        let missing = last_assistant_message(HarnessKind::Codex, &rollout, "turn-9").unwrap_err();
        assert!(missing.to_string().contains("turn-9"), "{missing}");
    }

    #[test]
    fn claude_answers_with_its_structured_output_or_its_last_message() {
        let dir = tempfile::tempdir().unwrap();
        let user = |uuid: &str| {
            serde_json::json!({"type":"user","uuid":uuid,"timestamp":"2026-11-16T10:00:00Z",
                "message":{"role":"user","content":"go"}})
        };
        let assistant = |content: serde_json::Value| {
            serde_json::json!({"type":"assistant","uuid":"a","timestamp":"2026-11-16T10:00:01Z",
                "message":{"id":"m","role":"assistant","stop_reason":"end_turn","content":content}})
        };
        let decision = serde_json::json!({"decision":"advance","summary":"done"});
        let source = transcript(
            dir.path(),
            &[
                user("text-turn"),
                assistant(serde_json::json!([{"type":"text","text":"Plain answer."}])),
                user("structured-turn"),
                assistant(serde_json::json!([{"type":"text","text":"Deciding."},
                    {"type":"tool_use","id":"t1","name":"StructuredOutput","input":decision}])),
            ],
        );
        assert_eq!(
            last_assistant_message(HarnessKind::Claude, &source, "text-turn").unwrap(),
            Some("Plain answer.".into())
        );
        let structured =
            last_assistant_message(HarnessKind::Claude, &source, "structured-turn").unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&structured.unwrap()).unwrap(),
            decision
        );
        assert!(last_assistant_message(HarnessKind::Claude, &source, "other").is_err());
    }

    #[test]
    fn a_completed_turn_whose_transcript_is_gone_fails_and_says_where() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        let session = store.test_session("conversation", &input);
        assert_eq!(input_final_answer(&store, &input).unwrap(), None);
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider='codex' WHERE id=?1",
                [&session.id],
            )
            .unwrap();
        loopflow_storage::test_fixture::record_captured_start(&store, &session.id, "turn");
        store
            .record_session_event(
                &session.id,
                &"thread".into(),
                "turn",
                crate::session::SessionEventKind::Completed,
                &serde_json::json!({"status":"completed"}),
            )
            .unwrap();
        let gone = format!("{:#}", input_final_answer(&store, &input).unwrap_err());
        assert!(
            gone.contains("thread") && gone.contains("is gone"),
            "{gone}"
        );
    }
}
