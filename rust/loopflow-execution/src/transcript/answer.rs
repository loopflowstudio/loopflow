//! A turn's final answer, read from the transcript its provider keeps. A
//! transcript that is gone, unreadable or without the turn is an error naming
//! the file, never an empty answer. The turn's row in history keeps a copy
//! for people, read only when the transcript is gone; routing never reads it.

use anyhow::{anyhow, Context, Result};
use loopflow_model::chat::types::ConversationItem;
use loopflow_storage::sqlite::SqliteStore;

use super::{locate, Gap, Located};
use loopflow_harness::transcript::{json_lines, read, Source, Turn};
use loopflow_harness::HarnessKind;

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

/// The final answer of `input` for a person reading it later: the
/// transcript's, or when the transcript cannot answer, the copy its turn's
/// row in history kept.
///
/// # Errors
/// When the transcript cannot answer and history kept no copy.
pub fn input_final_answer_for_people(store: &SqliteStore, input: &str) -> Result<Option<String>> {
    input_final_answer(store, input).or_else(|error| {
        let kept = store.input_recorded_answer(input)?;
        kept.map(Some).ok_or(error)
    })
}

/// What the agent said last in turn `turn` of a transcript, one reader per
/// provider: Codex's own record of the turn's last message, Claude's
/// structured answer or final message, and the final message of the others.
/// `None` when the turn ended without one.
///
/// # Errors
/// When the transcript cannot be read or holds no such turn.
fn last_assistant_message(
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
pub(crate) fn final_message(turn: &Turn) -> Option<String> {
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

    use super::{input_final_answer, input_final_answer_for_people, last_assistant_message};
    use loopflow_harness::transcript::Source;
    use loopflow_harness::HarnessKind;

    fn testdata(name: &str) -> Source {
        Source::File(loopflow_harness::transcript::testdata(name))
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
        let captured = store
            .session(&session.id)
            .unwrap()
            .unwrap()
            .captured
            .unwrap();
        let completed = serde_json::json!({"status":"completed"});
        store
            .record_answered_turn_end(captured, &"thread".into(), "turn", 1, &completed)
            .unwrap();
        let gone = format!("{:#}", input_final_answer(&store, &input).unwrap_err());
        assert!(
            gone.contains("thread") && gone.contains("is gone"),
            "{gone}"
        );
        // A person still reads the copy the turn's row kept; routing never does.
        store.record_turn_final_message(&session.id, &"thread".into(), "turn", "Kept answer.");
        assert!(input_final_answer(&store, &input).is_err());
        assert_eq!(
            input_final_answer_for_people(&store, &input).unwrap(),
            Some("Kept answer.".into())
        );
    }

    #[test]
    fn an_answer_needs_only_the_input_row_and_the_transcript() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        let session = store.test_session("conversation", &input);
        let thread = "0199a213-81c0-7800-8aa1-bbab2a035a53";
        let account = home.path().join("claude-home");
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE agent_sessions SET provider='claude',cwd='/work/repo' WHERE id=?1",
                [&session.id],
            )
            .unwrap();
            conn.execute(
                "UPDATE session_events SET provider_thread=?2
                 WHERE kind='captured' AND session_id=?1",
                rusqlite::params![session.id, thread],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO provider_accounts(provider,account_id,home,credential_state,created_at,updated_at)
                 VALUES('claude','fixture',?1,'missing',1,1)",
                [account.to_str().unwrap()],
            )
            .unwrap();
        }
        store.record_account_home_conversation(
            crate::provider_auth::Provider::Claude,
            thread,
            "fixture",
        );
        let captured_at: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT observed_at FROM session_events WHERE kind='captured' AND session_id=?1",
                [&session.id],
                |row| row.get(0),
            )
            .unwrap();
        let at = |offset: i64| {
            chrono::DateTime::from_timestamp(captured_at + offset, 0)
                .unwrap()
                .to_rfc3339()
        };
        let project =
            loopflow_harness::transcript::live::claude_project(&account, Path::new("/work/repo"));
        std::fs::create_dir_all(&project).unwrap();
        let row = |kind: &str, at: &str, content: serde_json::Value| {
            serde_json::json!({"type":kind,"uuid":format!("{kind}-{at}"),"timestamp":at,
                "message":{"id":at,"role":kind,"stop_reason":"end_turn","content":content}})
        };
        let write = |rows: &[serde_json::Value]| {
            let lines: Vec<String> = rows.iter().map(ToString::to_string).collect();
            std::fs::write(project.join(format!("{thread}.jsonl")), lines.join("\n")).unwrap();
        };
        let text = |text: &str| serde_json::json!([{"type":"text","text":text}]);
        // History kept no row for the turn: the input's row names it.
        write(&[
            row("user", &at(1), serde_json::json!("go")),
            row("assistant", &at(5), text("Answer.")),
        ]);
        assert_eq!(input_final_answer(&store, &input).unwrap(), None);
        let captured = store
            .session(&session.id)
            .unwrap()
            .unwrap()
            .captured
            .unwrap();
        store
            .record_answered_turn_end(
                captured,
                &thread.into(),
                &format!("user-{}", at(1)),
                captured_at + 5,
                &serde_json::json!({"status":"completed"}),
            )
            .unwrap();
        assert_eq!(
            input_final_answer(&store, &input).unwrap(),
            Some("Answer.".into())
        );
    }
}
