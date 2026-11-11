//! A conversation's per-turn usage and completion, written to the store from
//! its transcript. For Claude and Codex the transcript is the only writer of
//! `usage` rows: one per turn id, with the turn's tokens and model, written
//! when the turn ends and again when the provider exits. A turn that has a
//! row keeps it.
//!
//! At exit a last turn the transcript leaves open was cut off: its usage is
//! written as it stands, and it is completed as `interrupted` at the
//! transcript's last row. Completions are written only for turns Loopflow
//! recorded no end for, so a launch with no live stream still counts.
//!
//! A transcript gives no cost: Claude's `cost-state` row is a total, not a
//! turn's.

use std::path::Path;

use serde_json::{json, Value};

use super::live::TurnState;
use super::{seconds, unix_seconds, Turn};
use crate::harness::HarnessKind;
use crate::id::AgentSessionId;
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

/// How long after a turn's last transcript row its live completion may have
/// been recorded.
const LAG: i64 = 2;

/// When a Session's transcript is read into the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Moment {
    /// The provider says a turn ended; it goes on running.
    TurnEnd,
    /// The provider is gone.
    Exit,
}

/// Record what the transcript of `session`'s current conversation shows at
/// `moment`. Best-effort: the turn, or the exit, stands whatever the store or
/// the transcript says.
pub(crate) fn record_session(store: &SqliteStore, session: &str, moment: Moment) {
    if let Err(error) = record_current(store, session, moment) {
        tracing::warn!(%error, %session, "cannot record turns from the provider's transcript");
    }
}

fn record_current(store: &SqliteStore, session: &str, moment: Moment) -> anyhow::Result<usize> {
    let (Some(conversation), Some(launched)) = (
        store.current_conversation(session)?,
        store.session(session)?,
    ) else {
        return Ok(0);
    };
    let Some((harness, transcript)) = super::live::locate(store, &conversation, &launched.cwd)?
    else {
        return Ok(0);
    };
    let thread = &conversation.thread;
    Ok(match moment {
        Moment::TurnEnd => record_ended(store, session, harness, thread, &transcript)?,
        Moment::Exit => record(store, session, harness, thread, &transcript)?,
    })
}

/// Record the usage of each ended turn at the transcript's end; returns how
/// many were written. A provider can report a turn's end before its
/// transcript shows it, so the turn before is still due. One that began
/// before the rows read shows part of its usage; the exit read has it whole.
fn record_ended(
    store: &SqliteStore,
    session: &str,
    harness: HarnessKind,
    thread: &AgentSessionId,
    transcript: &Path,
) -> StoreResult<usize> {
    let mut written = 0;
    for turn in super::live::last_turns(harness, transcript) {
        if let (Some(_), Some(ended)) = (&turn.started_at, seconds(&turn.ended_at)) {
            written += usize::from(usage(store, session, thread, &turn, ended)?);
        }
    }
    Ok(written)
}

/// Record the usage of every turn in `thread`'s transcript, and the
/// completion of each `session` holds none for; returns how many turns were
/// written. The provider must be gone.
fn record(
    store: &SqliteStore,
    session: &str,
    harness: HarnessKind,
    thread: &AgentSessionId,
    transcript: &Path,
) -> StoreResult<usize> {
    let Some(read) = super::ended_reader(harness) else {
        return Ok(0);
    };
    let rows = super::json_lines(transcript).collect::<Vec<_>>();
    let turns = read(&rows);
    let last_row = || {
        rows.iter()
            .rev()
            .find_map(|row| unix_seconds(&row["timestamp"]))
    };
    let recorded = store.session_completions(session, thread)?;
    let mut written = 0;
    for turn in &turns {
        let (Some(id), Some(ended)) = (&turn.id, seconds(&turn.ended_at).or_else(last_row)) else {
            continue;
        };
        let mut wrote = usage(store, session, thread, turn, ended)?;
        let started = seconds(&turn.started_at);
        if !recorded
            .iter()
            .any(|(known, at)| known == id || covers(&turns, known, *at, started, ended))
        {
            let completed = completion(harness, turn, started, ended);
            store.record_turn_completion(session, thread, id, ended, &completed)?;
            wrote = true;
        }
        written += usize::from(wrote);
    }
    Ok(written)
}

fn usage(
    store: &SqliteStore,
    session: &str,
    thread: &AgentSessionId,
    turn: &Turn,
    ended: i64,
) -> StoreResult<bool> {
    match (&turn.id, &turn.receipt.usage) {
        (Some(id), usage @ Value::Object(_)) => {
            store.record_turn_usage(session, thread, id, ended, usage)
        }
        _ => Ok(false),
    }
}

/// Whether the completion recorded for `turn` at `at` accounts for a
/// transcript turn from `started` to `ended`. Loopflow's live recording folds
/// input queued during a turn into that turn, so one completion can span
/// several turns here: every one from its own turn's start until it was
/// observed.
fn covers(turns: &[Turn], turn: &str, at: i64, started: Option<i64>, ended: i64) -> bool {
    let from = turns
        .iter()
        .find(|known| known.id.as_deref() == Some(turn))
        .and_then(|known| seconds(&known.started_at));
    match (from, started) {
        (Some(from), Some(started)) => from <= started && ended <= at + LAG,
        _ => (at - ended).abs() <= LAG,
    }
}

fn completion(harness: HarnessKind, turn: &Turn, started: Option<i64>, ended: i64) -> Value {
    let status = match turn.state {
        Some(TurnState::Ended) => "completed",
        Some(TurnState::Interrupted | TurnState::Running) | None => "interrupted",
    };
    let mut detail = json!({
        "status": status,
        "started_at": started,
        "completed_at": ended,
        "duration_ms": turn.receipt.duration_ms,
    });
    if harness == HarnessKind::Claude {
        detail["result_id"] = Value::Null;
        detail["subtype"] = Value::Null;
        detail["errors"] = match turn.receipt.errors.is_empty() {
            true => Value::Null,
            false => turn.receipt.errors.clone().into(),
        };
    } else {
        detail["error"] = Value::Null;
    }
    detail
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionEventKind;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/transcript/testdata")
            .join(name)
    }

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        (home, store)
    }

    fn rows(store: &SqliteStore, kind: SessionEventKind) -> Vec<(String, i64, Value)> {
        store
            .session_history("conversation", 0, 0)
            .unwrap()
            .into_iter()
            .filter(|event| event.kind == kind)
            .map(|event| {
                (
                    event.provider_turn.unwrap(),
                    event.observed_at,
                    event.payload,
                )
            })
            .collect()
    }

    #[test]
    fn a_claude_transcript_writes_each_finished_turn_once() {
        let (_home, store) = store();
        let thread = AgentSessionId::from("5d1a7f0e-2c41-4f43-9a51-0d3b8f6c9e10");
        let path = fixture("claude_turn.jsonl");
        let record = || record(&store, "conversation", HarnessKind::Claude, &thread, &path);
        assert_eq!(record().unwrap(), 1);
        assert_eq!(record().unwrap(), 0);

        let usage = rows(&store, SessionEventKind::Usage);
        assert_eq!(usage.len(), 1);
        assert_eq!((usage[0].0.as_str(), usage[0].1), ("u1", 1_794_328_071));
        assert_eq!(
            usage[0].2,
            json!({"provider":"claude","final":true,"usage":{
                "input_tokens":4,"output_tokens":1100,"total_input_tokens":41200,
                "cache_read_tokens":38000,"cache_write_tokens":3196,"model":"claude-opus-5-5"}})
        );
        let reading: crate::chat::types::TurnUsage =
            serde_json::from_value(usage[0].2["usage"].clone()).unwrap();
        assert_eq!(reading.cost_usd, None);
        assert_eq!(
            rows(&store, SessionEventKind::Completed)[0].2,
            json!({"status":"completed","started_at":1_794_327_937,"completed_at":1_794_328_071,
                "duration_ms":134_000,"result_id":null,"subtype":null,"errors":null})
        );
    }

    #[test]
    fn a_turn_end_writes_ended_turns_once_and_leaves_completions_to_the_stream() {
        let (home, store) = store();
        let thread = AgentSessionId::from("5d1a7f0e-2c41-4f43-9a51-0d3b8f6c9e10");
        let whole = std::fs::read_to_string(fixture("claude_turn.jsonl")).unwrap();
        let path = home.path().join("transcript.jsonl");
        let ended =
            |path: &Path| record_ended(&store, "conversation", HarnessKind::Claude, &thread, path);

        // Still running: nothing yet.
        let running = whole.lines().take(6).collect::<Vec<_>>().join("\n");
        std::fs::write(&path, format!("{running}\n")).unwrap();
        assert_eq!(ended(&path).unwrap(), 0);

        std::fs::write(&path, &whole).unwrap();
        assert_eq!(ended(&path).unwrap(), 1);
        assert_eq!(ended(&path).unwrap(), 0);
        let usage = rows(&store, SessionEventKind::Usage);
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].2["usage"]["model"], "claude-opus-5-5");
        assert!(rows(&store, SessionEventKind::Completed).is_empty());

        // The exit read finds the row and adds only the completion nothing recorded.
        assert_eq!(
            record(&store, "conversation", HarnessKind::Claude, &thread, &path).unwrap(),
            1
        );
        assert_eq!(rows(&store, SessionEventKind::Usage).len(), 1);
        assert_eq!(rows(&store, SessionEventKind::Completed).len(), 1);
    }

    #[test]
    fn a_turn_end_the_rollout_does_not_show_yet_still_writes_the_turn_before() {
        let (home, store) = store();
        let thread = AgentSessionId::from("0199a213-81c0-7800-8aa1-bbab2a035a53");
        let whole = std::fs::read_to_string(fixture("codex_rollout.jsonl")).unwrap();
        let path = home.path().join("rollout.jsonl");
        // The second turn's `task_complete` is not written yet.
        let open = whole.lines().take(9).collect::<Vec<_>>().join("\n");
        std::fs::write(&path, format!("{open}\n")).unwrap();
        let ended = || record_ended(&store, "conversation", HarnessKind::Codex, &thread, &path);
        assert_eq!(ended().unwrap(), 1);
        assert_eq!(rows(&store, SessionEventKind::Usage)[0].0, "turn-1");
        std::fs::write(&path, &whole).unwrap();
        assert_eq!(ended().unwrap(), 1);
        assert_eq!(rows(&store, SessionEventKind::Usage)[1].0, "turn-2");
    }

    #[test]
    fn a_codex_rollout_writes_each_turn_with_its_baseline() {
        let (_home, store) = store();
        let thread = AgentSessionId::from("0199a213-81c0-7800-8aa1-bbab2a035a53");
        let written = record(
            &store,
            "conversation",
            HarnessKind::Codex,
            &thread,
            &fixture("codex_rollout.jsonl"),
        )
        .unwrap();
        assert_eq!(written, 2);
        let usage = rows(&store, SessionEventKind::Usage);
        assert_eq!(usage[1].0, "turn-2");
        assert_eq!(usage[1].2["baseline"]["inputTokens"], 20000);
        assert_eq!(usage[1].2["total"]["cachedInputTokens"], 30000);
        assert_eq!(usage[1].2["last"]["reasoningOutputTokens"], 100);
        assert_eq!(usage[1].2["peakInputTokens"], 25000);
        let completed = rows(&store, SessionEventKind::Completed);
        assert_eq!(completed[0].2["status"], "completed");
        assert_eq!(completed[1].1, 1_794_327_909);
    }

    #[test]
    fn a_turn_completed_while_it_ran_gets_its_usage_and_no_second_completion() {
        let (_home, store) = store();
        let thread = AgentSessionId::from("0199a213-81c0-7800-8aa1-bbab2a035a53");
        // The live stream recorded the first turn's end, under its own id.
        store
            .record_turn_completion(
                "conversation",
                &thread,
                "turn-1",
                1_794_327_609,
                &json!({"status":"completed"}),
            )
            .unwrap();
        let written = record(
            &store,
            "conversation",
            HarnessKind::Codex,
            &thread,
            &fixture("codex_rollout.jsonl"),
        )
        .unwrap();
        assert_eq!(written, 2);
        let turns = |kind| {
            rows(&store, kind)
                .into_iter()
                .map(|(turn, ..)| turn)
                .collect::<Vec<_>>()
        };
        assert_eq!(turns(SessionEventKind::Usage), ["turn-1", "turn-2"]);
        assert_eq!(turns(SessionEventKind::Completed), ["turn-1", "turn-2"]);
    }

    #[test]
    fn a_turn_the_provider_left_open_is_written_interrupted_once() {
        let (home, store) = store();
        let thread = AgentSessionId::from("5d1a7f0e-2c41-4f43-9a51-0d3b8f6c9e10");
        let path = home.path().join("transcript.jsonl");
        let row = |kind: &str, at: &str, rest: Value| {
            let mut row = json!({"type":kind,"userType":"external",
                "timestamp":format!("2026-11-10T16:{at}Z")});
            row.as_object_mut()
                .unwrap()
                .extend(rest.as_object().unwrap().clone());
            row.to_string()
        };
        let reply = |stop: &str, content: Value| {
            json!({"message":{"id":stop,"model":"claude-opus-5-5","stop_reason":stop,
                "content":[content],"usage":{"input_tokens":3,"output_tokens":40}}})
        };
        // Killed with a tool still running.
        let mut lines = vec![
            row(
                "user",
                "00:00",
                json!({"uuid":"u1","message":{"content":"Build it"}}),
            ),
            row(
                "assistant",
                "00:05",
                reply(
                    "tool_use",
                    json!({"type":"tool_use","id":"t1","name":"Bash","input":{}}),
                ),
            ),
            row("attachment", "00:30", json!({})),
        ];
        std::fs::write(&path, lines.join("\n")).unwrap();
        let record = || record(&store, "conversation", HarnessKind::Claude, &thread, &path);
        assert_eq!(record().unwrap(), 1);
        assert_eq!(record().unwrap(), 0);

        let usage = rows(&store, SessionEventKind::Usage);
        assert_eq!(usage.len(), 1);
        assert_eq!((usage[0].0.as_str(), usage[0].1), ("u1", 1_794_326_430));
        assert_eq!(usage[0].2["usage"]["output_tokens"], 40);
        let completed = rows(&store, SessionEventKind::Completed);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].2["status"], "interrupted");
        assert_eq!(completed[0].2["completed_at"], 1_794_326_430);

        // Resumed: the next request ends that turn and starts another.
        lines.push(row(
            "user",
            "12:00",
            json!({"uuid":"u2","message":{"content":"ok"}}),
        ));
        lines.push(row(
            "assistant",
            "12:09",
            reply("end_turn", json!({"type":"text","text":"Built."})),
        ));
        std::fs::write(&path, lines.join("\n")).unwrap();
        assert_eq!(record().unwrap(), 1);
        let completed = rows(&store, SessionEventKind::Completed);
        assert_eq!(
            completed
                .iter()
                .map(|(turn, _, detail)| (turn.as_str(), detail["status"].as_str().unwrap()))
                .collect::<Vec<_>>(),
            [("u1", "interrupted"), ("u2", "completed")]
        );
        assert_eq!(rows(&store, SessionEventKind::Usage).len(), 2);
    }

    #[test]
    fn one_live_completion_covers_the_turns_queued_into_it() {
        let turn = |id: &str, from: &str, to: &str| Turn {
            id: Some(id.into()),
            state: Some(TurnState::Ended),
            started_at: Some(from.into()),
            ended_at: Some(to.into()),
            model: None,
            usage: None,
            items: Vec::new(),
            receipt: Default::default(),
        };
        let turns = [
            turn("a", "2026-11-10T16:00:00Z", "2026-11-10T16:01:00Z"),
            turn("b", "2026-11-10T16:01:00Z", "2026-11-10T16:02:00Z"),
            turn("c", "2026-11-10T16:02:00Z", "2026-11-10T16:09:00Z"),
        ];
        let at = |turn: &Turn| (seconds(&turn.started_at), seconds(&turn.ended_at).unwrap());
        // `a`'s completion was observed a second after `b` ended.
        let observed = at(&turns[1]).1 + 1;
        let covered = |turn: &Turn| covers(&turns, "a", observed, at(turn).0, at(turn).1);
        assert!(covered(&turns[1]));
        assert!(!covered(&turns[2]));
    }
}
