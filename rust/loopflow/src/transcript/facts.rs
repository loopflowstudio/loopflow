//! A conversation's per-turn usage and completion, written to the store from
//! its transcript in the shape the live stream's receipts have, so a launch
//! with no live stream still counts toward its Task. A turn Loopflow already
//! recorded while it ran is left alone.
//!
//! This runs once the provider has exited, so a last run the transcript
//! leaves open (still going, or stopped on a question) was cut off: it is
//! written as `interrupted`, ended at the transcript's last row.
//!
//! A transcript gives no cost: Claude's `cost-state` row is a total, not a
//! turn's. Rows from here carry `"source":"transcript"` and no `cost_usd`.

use std::path::Path;

use serde_json::{json, Value};

use super::live::TurnState;
use super::{unix_seconds, Run};
use crate::harness::HarnessKind;
use crate::id::AgentSessionId;
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

/// How long after a turn's last transcript row its live completion may have
/// been recorded.
const LAG: i64 = 2;

/// Record every turn of `thread`'s transcript that `session` holds no
/// completion for; returns how many were written. The provider must be gone.
pub(crate) fn record(
    store: &SqliteStore,
    session: &str,
    harness: HarnessKind,
    thread: &AgentSessionId,
    transcript: &Path,
) -> StoreResult<usize> {
    let runs = super::runs(harness, transcript);
    let last_row = || {
        super::json_lines(transcript)
            .filter_map(|row| unix_seconds(&row["timestamp"]))
            .last()
    };
    let recorded = store.session_completions(session, thread)?;
    let mut written = 0;
    for run in &runs {
        let (Some(id), Some(ended)) = (&run.id, seconds(&run.ended_at).or_else(last_row)) else {
            continue;
        };
        let started = seconds(&run.started_at);
        if recorded
            .iter()
            .any(|(turn, at)| turn == id || covers(&runs, turn, *at, started, ended))
        {
            continue;
        }
        let usage = run.receipt.is_object().then(|| {
            let mut usage = run.receipt.clone();
            usage["source"] = "transcript".into();
            if harness == HarnessKind::Claude {
                usage["final"] = true.into();
            }
            usage
        });
        let completed = completion(harness, run, started, ended);
        store.record_transcript_turn(session, thread, id, ended, usage.as_ref(), &completed)?;
        written += 1;
    }
    Ok(written)
}

/// Whether the completion recorded for `turn` at `at` accounts for a run from
/// `started` to `ended`. Loopflow's live recording folds input queued during
/// a turn into that turn, so one completion can span several runs here: every
/// run from its own turn's start until it was observed.
fn covers(runs: &[Run], turn: &str, at: i64, started: Option<i64>, ended: i64) -> bool {
    let from = runs
        .iter()
        .find(|run| run.id.as_deref() == Some(turn))
        .and_then(|run| seconds(&run.started_at));
    match (from, started) {
        (Some(from), Some(started)) => from <= started && ended <= at + LAG,
        _ => (at - ended).abs() <= LAG,
    }
}

fn completion(harness: HarnessKind, run: &Run, started: Option<i64>, ended: i64) -> Value {
    let status = match run.state {
        TurnState::Ended => "completed",
        TurnState::Interrupted | TurnState::Running | TurnState::Asking => "interrupted",
    };
    let mut detail = json!({
        "status": status,
        "started_at": started,
        "completed_at": ended,
        "duration_ms": run.duration_ms,
        "source": "transcript",
    });
    if harness == HarnessKind::Claude {
        detail["result_id"] = Value::Null;
        detail["subtype"] = Value::Null;
        detail["errors"] = match run.errors.is_empty() {
            true => Value::Null,
            false => run.errors.clone().into(),
        };
    } else {
        detail["error"] = Value::Null;
    }
    detail
}

fn seconds(at: &Option<String>) -> Option<i64> {
    unix_seconds(&Value::String(at.clone()?))
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
    fn a_claude_transcript_writes_each_finished_turn_once_in_the_live_shape() {
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
            json!({"provider":"claude","final":true,"source":"transcript","usage":{
                "input_tokens":4,"output_tokens":1100,"total_input_tokens":41200,
                "cache_read_tokens":38000,"cache_write_tokens":3196,"model":"claude-opus-5-5"}})
        );
        let reading: crate::chat::types::TurnUsage =
            serde_json::from_value(usage[0].2["usage"].clone()).unwrap();
        assert_eq!(reading.cost_usd, None);
        assert_eq!(
            rows(&store, SessionEventKind::Completed)[0].2,
            json!({"status":"completed","started_at":1_794_327_937,"completed_at":1_794_328_071,
                "duration_ms":134_000,"source":"transcript","result_id":null,"subtype":null,
                "errors":null})
        );
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
    fn a_turn_recorded_while_it_ran_is_not_written_again() {
        let (_home, store) = store();
        let thread = AgentSessionId::from("0199a213-81c0-7800-8aa1-bbab2a035a53");
        // The live stream recorded the first turn, under its own id, as it ended.
        store
            .record_transcript_turn(
                "conversation",
                &thread,
                "turn-1",
                1_794_327_609,
                None,
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
        assert_eq!(written, 1);
        assert_eq!(rows(&store, SessionEventKind::Usage)[0].0, "turn-2");
    }

    #[test]
    fn a_run_the_provider_left_open_is_written_interrupted_once() {
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

        // Resumed: the next request ends the same run, under the same id.
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
    fn one_live_completion_covers_the_runs_queued_into_it() {
        let run = |id: &str, from: &str, to: &str| Run {
            id: Some(id.into()),
            state: TurnState::Ended,
            started_at: Some(from.into()),
            ended_at: Some(to.into()),
            model: None,
            usage: Default::default(),
            receipt: Value::Null,
            duration_ms: None,
            errors: Vec::new(),
        };
        let runs = [
            run("a", "2026-11-10T16:00:00Z", "2026-11-10T16:01:00Z"),
            run("b", "2026-11-10T16:01:00Z", "2026-11-10T16:02:00Z"),
            run("c", "2026-11-10T16:02:00Z", "2026-11-10T16:09:00Z"),
        ];
        let at = |run: &Run| (seconds(&run.started_at), seconds(&run.ended_at).unwrap());
        // `a`'s completion was observed a second after `b` ended.
        let observed = at(&runs[1]).1 + 1;
        let covered = |run: &Run| covers(&runs, "a", observed, at(run).0, at(run).1);
        assert!(covered(&runs[1]));
        assert!(!covered(&runs[2]));
    }
}
