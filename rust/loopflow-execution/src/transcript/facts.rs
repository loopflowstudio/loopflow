//! A conversation's per-turn usage and completion, written to `history.turns`
//! from its transcript. The transcript is the only writer of a turn's usage:
//! its tokens and model, written when the turn ends and again when the
//! provider exits. A turn that has a reading keeps it.
//!
//! At exit a last turn the transcript leaves open was cut off: its usage is
//! written as it stands, and it is completed as `interrupted` at the
//! transcript's last row. Completions are written only for turns Loopflow
//! recorded no end for, so a launch with no live stream still counts.
//!
//! Pi and OpenCode report each model call's cost, so their rows carry the
//! turn's; Claude's `cost-state` row is a total, not a turn's, and Codex
//! reports none.

use serde_json::{json, Value};

use loopflow_harness::transcript::live::TurnState;
use loopflow_harness::transcript::{seconds, Source, Turn};
use loopflow_harness::HarnessKind;
use loopflow_model::id::AgentSessionId;
use loopflow_storage::sqlite::{SqliteStore, TurnSpan};

/// How long after a turn's last transcript row its live completion may have
/// been recorded.
const LAG: i64 = 2;

/// When a Session's transcript is read into the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    /// The provider says a turn ended; it goes on running.
    TurnEnd,
    /// The provider is gone.
    Exit,
}

/// Record what the transcript of `session`'s current conversation shows at
/// `moment`. Best-effort: the turn, or the exit, stands whatever the store or
/// the transcript says.
pub fn record_session(store: &SqliteStore, session: &str, moment: Moment) {
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
    let Some((harness, source)) = super::live::locate(store, &conversation, &launched.cwd)? else {
        return Ok(0);
    };
    let thread = &conversation.thread;
    match moment {
        Moment::TurnEnd => record_ended(store, session, harness, thread, &source),
        Moment::Exit => record(store, session, harness, thread, &source),
    }
}

/// Record the usage of each ended turn at the transcript's end; returns how
/// many were written. A provider can report a turn's end before its
/// transcript shows it, so the turn before is still due. One that began
/// before the rows read shows only part of its usage, so it is left to the
/// exit read, which has it whole.
fn record_ended(
    store: &SqliteStore,
    session: &str,
    harness: HarnessKind,
    thread: &AgentSessionId,
    source: &Source,
) -> anyhow::Result<usize> {
    let turns = loopflow_harness::transcript::live::last_turns(harness, source);
    let recorded = if harness == HarnessKind::Pi {
        store.session_turn_spans(session, thread)?
    } else {
        Vec::new()
    };
    let mut written = 0;
    for turn in &turns {
        let (Some(own), Some(_), Some(ended)) = (
            turn.id.as_deref(),
            &turn.started_at,
            seconds(&turn.ended_at),
        ) else {
            continue;
        };
        let id = pi_turn_id(&recorded, &turns, own, turn, ended);
        written += usize::from(usage(store, session, thread, id, turn, ended));
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
    source: &Source,
) -> anyhow::Result<usize> {
    let turns = loopflow_harness::transcript::read(harness, source)?;
    let last_row = || source.last_written();
    let recorded = store.session_turn_spans(session, thread)?;
    let mut written = 0;
    for turn in &turns {
        let (Some(own), Some(ended)) = (&turn.id, seconds(&turn.ended_at).or_else(last_row)) else {
            continue;
        };
        let id = if harness == HarnessKind::Pi {
            pi_turn_id(&recorded, &turns, own, turn, ended)
        } else {
            own
        };
        let mut wrote = usage(store, session, thread, id, turn, ended);
        let started = seconds(&turn.started_at);
        if !recorded.iter().any(|span| {
            span.ended_at
                .is_some_and(|at| span.turn == id || covers(&turns, &span.turn, at, started, ended))
        }) {
            let completed = completion(harness, turn, started, ended);
            store.record_turn_completion(session, thread, id, ended, &completed);
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
    id: &str,
    turn: &Turn,
    ended: i64,
) -> bool {
    if let Some(message) = super::answer::final_message(turn) {
        store.record_turn_final_message(session, thread, id, &message);
    }
    match &turn.receipt.usage {
        usage @ Value::Object(_) => store.record_turn_usage(session, thread, id, ended, usage),
        _ => false,
    }
}

/// Pi names turns by session-file entry id. Its RPC driver uses the request
/// id, so an otherwise unknown entry joins the recorded turn spanning it.
fn pi_turn_id<'a>(
    recorded: &'a [TurnSpan],
    turns: &[Turn],
    own: &'a str,
    turn: &Turn,
    ended: i64,
) -> &'a str {
    if recorded.iter().any(|span| span.turn == own) {
        return own;
    }
    let started = seconds(&turn.started_at);
    recorded
        .iter()
        .filter(|span| {
            !turns
                .iter()
                .any(|read| read.id.as_deref() == Some(&span.turn))
        })
        .filter(|span| match (span.started_at, started) {
            (Some(from), Some(started)) => {
                from <= started + LAG && span.ended_at.is_none_or(|to| ended <= to + LAG)
            }
            _ => span.ended_at.is_some_and(|to| (to - ended).abs() <= LAG),
        })
        .max_by_key(|span| span.started_at)
        .map_or(own, |span| &span.turn)
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
        TurnState::Ended => "completed",
        TurnState::Interrupted | TurnState::Running => "interrupted",
    };
    let mut detail = json!({
        "status": status,
        "started_at": started,
        "completed_at": ended,
        "duration_ms": turn.receipt.duration_ms,
    });
    harness
        .harness()
        .transcript_completion_detail(turn, &mut detail);
    detail
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use loopflow_harness::transcript::testdata as fixture;

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        (home, store)
    }

    /// Each turn with usage: its id, when the usage was read, and the turn.
    fn usage(store: &SqliteStore) -> Vec<(String, i64, crate::session::Turn)> {
        turns(store, |turn| {
            turn.usage.as_ref().map(|usage| usage.observed_at)
        })
    }

    /// Each turn that ended: its id, when, and the turn.
    fn finished(store: &SqliteStore) -> Vec<(String, i64, crate::session::Turn)> {
        turns(store, |turn| turn.ended_at)
    }

    fn turns(
        store: &SqliteStore,
        at: impl Fn(&crate::session::Turn) -> Option<i64>,
    ) -> Vec<(String, i64, crate::session::Turn)> {
        store
            .session_turns("conversation")
            .unwrap()
            .into_iter()
            .filter_map(|turn| Some((turn.turn.clone(), at(&turn)?, turn)))
            .collect()
    }

    fn reading(turn: &crate::session::Turn) -> &loopflow_model::chat::types::TurnUsage {
        &turn.usage.as_ref().unwrap().usage
    }

    #[test]
    fn a_claude_transcript_writes_each_finished_turn_once() {
        let (_home, store) = store();
        let thread = AgentSessionId::from("5d1a7f0e-2c41-4f43-9a51-0d3b8f6c9e10");
        let path = fixture("claude_turn.jsonl");
        let record = || {
            record(
                &store,
                "conversation",
                HarnessKind::Claude,
                &thread,
                &Source::File(path.clone()),
            )
        };
        assert_eq!(record().unwrap(), 1);
        assert_eq!(record().unwrap(), 0);

        let usage = usage(&store);
        assert_eq!(usage.len(), 1);
        assert_eq!((usage[0].0.as_str(), usage[0].1), ("u1", 1_794_328_071));
        let turn = &usage[0].2;
        assert_eq!(
            *reading(turn),
            loopflow_model::chat::types::TurnUsage {
                input_tokens: Some(4),
                output_tokens: Some(1100),
                total_input_tokens: Some(41200),
                cache_read_tokens: Some(38000),
                cache_write_tokens: Some(3196),
                model: Some("claude-opus-5-5".into()),
                ..Default::default()
            }
        );
        assert!(turn.usage.as_ref().unwrap().final_reading);
        assert_eq!(turn.provider.as_deref(), Some("claude"));
        assert_eq!(
            (
                turn.ended_at,
                turn.status.as_deref(),
                turn.duration_ms,
                &turn.error
            ),
            (Some(1_794_328_071), Some("completed"), Some(134_000), &None)
        );
    }

    #[test]
    fn a_turn_end_writes_ended_turns_once_and_leaves_completions_to_the_stream() {
        let (home, store) = store();
        let thread = AgentSessionId::from("5d1a7f0e-2c41-4f43-9a51-0d3b8f6c9e10");
        let whole = std::fs::read_to_string(fixture("claude_turn.jsonl")).unwrap();
        let path = home.path().join("transcript.jsonl");
        let ended = |path: &Path| {
            record_ended(
                &store,
                "conversation",
                HarnessKind::Claude,
                &thread,
                &Source::File(path.into()),
            )
            .unwrap()
        };

        // Still running: nothing yet.
        let running = whole.lines().take(6).collect::<Vec<_>>().join("\n");
        std::fs::write(&path, format!("{running}\n")).unwrap();
        assert_eq!(ended(&path), 0);

        std::fs::write(&path, &whole).unwrap();
        assert_eq!(ended(&path), 1);
        assert_eq!(ended(&path), 0);
        let read = usage(&store);
        assert_eq!(read.len(), 1);
        assert_eq!(
            reading(&read[0].2).model.as_deref(),
            Some("claude-opus-5-5")
        );
        assert!(finished(&store).is_empty());

        // The exit read finds the row and adds only the completion nothing recorded.
        assert_eq!(
            record(
                &store,
                "conversation",
                HarnessKind::Claude,
                &thread,
                &Source::File(path.clone())
            )
            .unwrap(),
            1
        );
        assert_eq!(usage(&store).len(), 1);
        assert_eq!(finished(&store).len(), 1);
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
        let ended = || {
            record_ended(
                &store,
                "conversation",
                HarnessKind::Codex,
                &thread,
                &Source::File(path.clone()),
            )
            .unwrap()
        };
        assert_eq!(ended(), 1);
        assert_eq!(usage(&store)[0].0, "turn-1");
        std::fs::write(&path, &whole).unwrap();
        assert_eq!(ended(), 1);
        assert_eq!(usage(&store)[1].0, "turn-2");
    }

    #[test]
    fn a_codex_rollout_writes_each_turn_as_its_own_counts() {
        let (_home, store) = store();
        let thread = AgentSessionId::from("0199a213-81c0-7800-8aa1-bbab2a035a53");
        let written = record(
            &store,
            "conversation",
            HarnessKind::Codex,
            &thread,
            &Source::File(fixture("codex_rollout.jsonl")),
        )
        .unwrap();
        assert_eq!(written, 2);
        let usage = usage(&store);
        assert_eq!(usage[1].0, "turn-2");
        // The thread's totals less where the second turn began.
        let second = reading(&usage[1].2);
        assert_eq!(second.total_input_tokens, Some(25000));
        assert_eq!(second.cache_read_tokens, Some(18000));
        assert_eq!(second.input_tokens, Some(7000));
        assert_eq!(second.output_tokens, Some(500));
        assert_eq!(second.reasoning_tokens, Some(100));
        assert_eq!(second.peak_input_tokens, Some(25000));
        assert!(!usage[1].2.usage.as_ref().unwrap().partial);
        let completed = finished(&store);
        assert_eq!(completed[0].2.status.as_deref(), Some("completed"));
        assert_eq!(completed[1].1, 1_794_327_909);
    }

    #[test]
    fn a_turn_completed_while_it_ran_gets_its_usage_and_no_second_completion() {
        let (_home, store) = store();
        let thread = AgentSessionId::from("0199a213-81c0-7800-8aa1-bbab2a035a53");
        // The live stream recorded the first turn's end, under its own id.
        store.record_turn_completion(
            "conversation",
            &thread,
            "turn-1",
            1_794_327_609,
            &json!({"status":"completed"}),
        );
        let written = record(
            &store,
            "conversation",
            HarnessKind::Codex,
            &thread,
            &Source::File(fixture("codex_rollout.jsonl")),
        )
        .unwrap();
        assert_eq!(written, 2);
        let ids = |read: Vec<(String, i64, crate::session::Turn)>| {
            read.into_iter().map(|(turn, ..)| turn).collect::<Vec<_>>()
        };
        assert_eq!(ids(usage(&store)), ["turn-1", "turn-2"]);
        assert_eq!(ids(finished(&store)), ["turn-1", "turn-2"]);
        // The live end stands; the transcript did not end it again.
        assert_eq!(finished(&store)[0].1, 1_794_327_609);
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
        let record = || {
            record(
                &store,
                "conversation",
                HarnessKind::Claude,
                &thread,
                &Source::File(path.clone()),
            )
        };
        assert_eq!(record().unwrap(), 1);
        assert_eq!(record().unwrap(), 0);

        let read = usage(&store);
        assert_eq!(read.len(), 1);
        assert_eq!((read[0].0.as_str(), read[0].1), ("u1", 1_794_326_430));
        assert_eq!(reading(&read[0].2).output_tokens, Some(40));
        let completed = finished(&store);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].2.status.as_deref(), Some("interrupted"));
        assert_eq!(completed[0].1, 1_794_326_430);

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
        let completed = finished(&store);
        assert_eq!(
            completed
                .iter()
                .map(|(turn, _, read)| (turn.as_str(), read.status.as_deref().unwrap()))
                .collect::<Vec<_>>(),
            [("u1", "interrupted"), ("u2", "completed")]
        );
        assert_eq!(usage(&store).len(), 2);
    }

    #[test]
    fn one_live_completion_covers_the_turns_queued_into_it() {
        let turn = |id: &str, from: &str, to: &str| Turn {
            id: Some(id.into()),
            state: TurnState::Ended,
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

    #[test]
    fn a_pi_session_writes_each_turn_with_its_cost_and_completion() {
        let (home, store) = store();
        let path = home.path().join("session.jsonl");
        let thread = AgentSessionId::from(path.display().to_string().as_str());
        let entries = [
            json!({"type":"session","version":3,"id":"s","cwd":"/work"}),
            json!({"type":"message","id":"u1","parentId":"s","timestamp":"2026-11-10T16:25:00Z",
                "message":{"role":"user","content":"List files"}}),
            json!({"type":"message","id":"a1","parentId":"u1","timestamp":"2026-11-10T16:25:09Z",
                "message":{"role":"assistant","model":"gemini-4-argon","stopReason":"stop",
                    "usage":{"input":10,"output":5,"cacheRead":90,"cacheWrite":0,
                        "cost":{"total":0.25}},
                    "content":[{"type":"text","text":"Two files."}]}}),
        ];
        let text = entries
            .iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>();
        std::fs::write(&path, text).unwrap();
        let source = Source::File(path);
        let record = || record(&store, "conversation", HarnessKind::Pi, &thread, &source);
        assert_eq!(record().unwrap(), 1);
        assert_eq!(record().unwrap(), 0);
        let usage = usage(&store);
        assert_eq!((usage[0].0.as_str(), usage[0].1), ("u1", 1_794_327_909));
        assert_eq!(usage[0].2.provider.as_deref(), Some("pi"));
        assert_eq!(reading(&usage[0].2).cost_usd, Some(0.25));
        assert_eq!(reading(&usage[0].2).total_input_tokens, Some(100));
        let completed = finished(&store);
        assert_eq!(completed[0].2.status.as_deref(), Some("completed"));
    }
    /// Loopflow names a Pi turn by its request id; the session file by its
    /// entry id. The usage belongs on the turn Loopflow recorded.
    #[test]
    fn a_pi_turn_loopflow_recorded_takes_the_session_files_usage() {
        let (home, store) = store();
        let path = home.path().join("session.jsonl");
        let thread = AgentSessionId::from(path.display().to_string().as_str());
        let message = |id: &str, parent: &str, at: &str, message: Value| {
            json!({"type":"message","id":id,"parentId":parent,
                "timestamp":format!("2026-11-10T16:{at}Z"),"message":message})
        };
        let answer = |text: &str| {
            json!({"role":"assistant","model":"gemini-4-argon","stopReason":"stop",
                "usage":{"input":10,"output":5,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.25}},
                "content":[{"type":"text","text":text}]})
        };
        let entries = [
            json!({"type":"session","version":3,"id":"s","cwd":"/work"}),
            message(
                "u1",
                "s",
                "25:03",
                json!({"role":"user","content":"List files"}),
            ),
            message("a1", "u1", "25:09", answer("Two files.")),
            message(
                "u2",
                "a1",
                "40:00",
                json!({"role":"user","content":"And now?"}),
            ),
            message("a2", "u2", "40:05", answer("Three.")),
        ];
        let text = entries
            .iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>();
        std::fs::write(&path, text).unwrap();
        let source = Source::File(path);
        // Loopflow sent the first request before Pi wrote it down, and saw it end.
        store.record_turn_start("conversation", &thread, "request-1", 1_794_327_900);
        store.record_turn_completion(
            "conversation",
            &thread,
            "request-1",
            1_794_327_910,
            &json!({"status":"completed"}),
        );
        // The second is still open when its end is read.
        store.record_turn_start("conversation", &thread, "request-2", 1_794_328_799);
        assert_eq!(
            record_ended(&store, "conversation", HarnessKind::Pi, &thread, &source).unwrap(),
            2
        );
        let record = || record(&store, "conversation", HarnessKind::Pi, &thread, &source);
        assert_eq!(record().unwrap(), 1);
        assert_eq!(record().unwrap(), 0);

        let usage = usage(&store);
        assert_eq!(
            usage.iter().map(|read| read.0.as_str()).collect::<Vec<_>>(),
            ["request-1", "request-2"]
        );
        assert_eq!(reading(&usage[1].2).output_tokens, Some(5));
        // Nothing was written under the session file's names.
        assert_eq!(store.session_turns("conversation").unwrap().len(), 2);
        let completed = finished(&store);
        assert_eq!(
            (completed[0].0.as_str(), completed[0].1),
            ("request-1", 1_794_327_910)
        );
        assert_eq!(
            (completed[1].0.as_str(), completed[1].1),
            ("request-2", 1_794_328_805)
        );
    }
}
