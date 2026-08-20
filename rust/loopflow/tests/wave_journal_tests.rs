//! The wave journal spine, end to end: a wave server's thread survives a
//! restart because the thread is a fold over the append-only journal. Also
//! covers the server's read surface over that spine (`/health`).

use std::path::Path;
use std::sync::Arc;

use loopflow::chat::turns::ChatRole;
use loopflow::chat::types::{ConversationItem, Lifecycle};
use loopflow::wave::journal::{fold_thread, journal_path, Journal, MessageOp};
use loopflow::wave::playhead::BodyProvenance;
use loopflow::wave::runtime::WaveRuntime;
use loopflow::wave::server::{self, ResidentDoor};
use loopflow::wave::state::LoopState;
use loopflow::wave::wire::ResidentDelta;

/// One complete resident turn, as the loop emits it after a pass: an
/// item, the pass's reply text, then the finalized boundary.
fn resident_turn_deltas() -> Vec<ResidentDelta> {
    vec![
        ResidentDelta::TurnOpened { answers: vec![] },
        ResidentDelta::TurnItem {
            item: ConversationItem::Command {
                id: "item-0".into(),
                command: vec!["cargo test".into()],
                cwd: String::new(),
                status: Lifecycle::Completed,
                output: None,
                exit_code: Some(0),
                duration_ms: None,
            },
        },
        ResidentDelta::TurnText {
            text: "Implemented the feature.".into(),
        },
        ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            reason: None,
        },
    ]
}

/// Run one resident turn through the production pipeline (resident wire
/// deltas → the listener's fold), as the resident door would.
fn run_resident_turn(runtime: Arc<WaveRuntime>, deltas: Vec<ResidentDelta>) {
    for delta in deltas {
        runtime.apply_resident_delta(delta);
    }
}

fn turn_seq(id: &str) -> u64 {
    id.strip_prefix("turn-")
        .and_then(|n| n.parse().ok())
        .expect("turn id from journal seq")
}

fn open_wave(repo: &Path) -> Arc<WaveRuntime> {
    WaveRuntime::open("ship".into(), repo.to_path_buf()).expect("open runtime")
}

fn body_for_current(runtime: &WaveRuntime, body_id: &str) -> BodyProvenance {
    let step = runtime
        .playhead()
        .and_then(|playhead| playhead.now)
        .expect("current playhead step");
    BodyProvenance {
        body_id: body_id.to_string(),
        invocation_id: step.invocation_id,
        step_index: step.index,
        flow: step.flow,
        step: step.step,
        iteration: step.iteration,
        session_id: Some("session-1".to_string()),
        harness: Some("codex".to_string()),
        model: Some("gpt-5".to_string()),
        host: "test-host".to_string(),
        worktree: runtime.repo_root().display().to_string(),
        started_at: "2026-07-09T12:00:00Z".to_string(),
        ended_at: None,
        termination_reason: None,
    }
}

#[tokio::test]
async fn restart_replays_thread_and_turn_ids_continue() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // First life: a user message and a real finalized turn.
    let before = {
        let rt = open_wave(tmp.path());
        rt.deliver(MessageOp::Message, "please build the feature".into())
            .expect("user turn");
        run_resident_turn(rt.clone(), resident_turn_deltas());
        let before = rt.thread_snapshot();
        assert_eq!(before.len(), 2);
        before
    };

    // Second life, same journal: the thread is intact, byte for byte.
    let rt = open_wave(tmp.path());
    assert_eq!(
        rt.thread_snapshot(),
        before,
        "restart keeps the full thread"
    );
    assert_eq!(rt.loop_state(), LoopState::Idle);

    // And new turn ids continue the journal's seq domain monotonically.
    let max_before = before.iter().map(|t| turn_seq(&t.id)).max().unwrap();
    let next = rt
        .deliver(MessageOp::Message, "still there?".into())
        .expect("user turn");
    assert!(
        turn_seq(&next.id) > max_before,
        "new turn id {} continues past {max_before}",
        next.id
    );
}

#[tokio::test]
async fn fold_of_journal_equals_the_turns_the_live_pipeline_built() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let rt = open_wave(tmp.path());
    run_resident_turn(rt.clone(), resident_turn_deltas());
    let live = rt.thread_snapshot();

    // Independent fold of the raw journal — no runtime involved.
    let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen journal");
    let fold = fold_thread(&events);
    assert!(fold.open.is_empty());
    assert_eq!(fold.turns, live, "fold(journal) == live thread");

    assert_eq!(live.len(), 1);
    let turn = &live[0];
    assert_eq!(turn.role, ChatRole::Assistant);
    assert_eq!(turn.status, Lifecycle::Completed);
    assert!(turn.text.contains("Implemented the feature."));
    assert_eq!(turn.items.len(), 1);
}

#[tokio::test]
async fn crashed_open_turn_is_finalized_failed_on_reboot() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // A server that dies mid-turn leaves started/item events with no finish.
    {
        let (mut journal, _) = Journal::open(&journal_path(tmp.path(), "ship")).expect("open");
        use loopflow::wave::journal::EventKind;
        journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            answers: vec![],
            body: None,
        });
        journal.append(|_| EventKind::TurnItem {
            turn_id: "turn-1".into(),
            item: ConversationItem::Message {
                id: "text-0".into(),
                text: "half a thought".into(),
                phase: None,
            },
        });
        journal.append(|_| EventKind::LoopState {
            from: LoopState::Idle,
            to: LoopState::Turning {
                turn_id: "turn-1".into(),
            },
            reason: "turn opened".into(),
        });
    }

    let rt = open_wave(tmp.path());
    let thread = rt.thread_snapshot();
    assert_eq!(thread.len(), 1);
    assert_eq!(
        thread[0].status,
        Lifecycle::Failed,
        "janitor closed the crash tail"
    );
    assert_eq!(thread[0].text, "half a thought");
    assert_eq!(rt.loop_state(), LoopState::Idle, "janitor settled the loop");
}

#[tokio::test]
async fn restart_interrupts_the_abandoned_body_without_advancing_the_playhead() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let first_step = {
        let rt = open_wave(tmp.path());
        let view = rt.ensure_playhead().expect("initialize playhead");
        let first_step = view.now.expect("first step");
        rt.start_body(body_for_current(&rt, "body-1"))
            .expect("start body");
        rt.apply_resident_delta(ResidentDelta::TurnOpened { answers: vec![] });
        first_step
    };

    let rt = open_wave(tmp.path());
    let playhead = rt.playhead().expect("replayed playhead");
    assert!(playhead.active.is_none(), "abandoned body was closed");
    assert_eq!(
        playhead.now.expect("same logical step"),
        first_step,
        "a process crash retries instead of silently advancing"
    );
    let turn = rt
        .thread_snapshot()
        .pop()
        .expect("recovered assistant turn");
    assert_eq!(turn.status, Lifecycle::Failed);
    let body = turn.body.expect("turn keeps body provenance");
    assert_eq!(body.body_id, "body-1");
    assert_eq!(
        body.termination_reason.as_deref(),
        Some("startup janitor: body abandoned by server restart")
    );
}

#[tokio::test]
async fn corrupt_trailing_line_is_tolerated_on_reboot() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let before = {
        let rt = open_wave(tmp.path());
        rt.deliver(MessageOp::Message, "kept message".into())
            .expect("user turn");
        rt.thread_snapshot()
    };

    // Crash mid-write: torn JSON at the tail.
    let path = journal_path(tmp.path(), "ship");
    let mut raw = std::fs::read_to_string(&path).expect("read journal");
    raw.push_str(r#"{"v":1,"seq":99,"at":"2026-07"#);
    std::fs::write(&path, &raw).expect("tear the tail");

    let rt = open_wave(tmp.path());
    assert_eq!(
        rt.thread_snapshot(),
        before,
        "thread intact past the torn tail"
    );
    // The journal still appends cleanly after truncation.
    rt.deliver(MessageOp::Message, "after the crash".into())
        .expect("user turn");
    assert_eq!(rt.thread_snapshot().len(), 2);
}

#[tokio::test]
async fn illegal_loop_transition_is_refused() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let rt = open_wave(tmp.path());
    let path = journal_path(tmp.path(), "ship");
    let (_, before) = Journal::open(&path).expect("open journal before transition");

    assert!(!rt.transition(
        LoopState::Interrupting {
            turn_id: "turn-1".into()
        },
        "nothing to interrupt"
    ));
    assert_eq!(rt.loop_state(), LoopState::Idle, "state untouched");
    // Refused moves add no trace beyond the runtime's durable epoch.
    let (_, after) = Journal::open(&path).expect("open journal after transition");
    assert_eq!(after.len(), before.len());
}

/// `/health` splits listener liveness (`status`, always `serving`) from the
/// resident's condition (`loop`: null while no resident was ever spawned or
/// attached, then the loop-state name).
#[tokio::test]
async fn health_reports_listener_liveness_and_the_loop_state() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let rt = open_wave(tmp.path());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = server::router(
        rt.clone(),
        ResidentDoor::new("test-token"),
        None,
        None,
        server::ShutdownDoor::new(),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    let body: serde_json::Value = reqwest::get(format!("http://{addr}/health"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["status"], "serving");
    assert!(
        body["loop_state"].is_null(),
        "no resident yet: a dormant listener"
    );

    rt.set_resident_expected();
    let body: serde_json::Value = reqwest::get(format!("http://{addr}/health"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["loop_state"], "idle");

    rt.transition(
        LoopState::Turning {
            turn_id: "turn-1".into(),
        },
        "test turn",
    );
    let body: serde_json::Value = reqwest::get(format!("http://{addr}/health"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["status"], "serving", "listener liveness is constant");
    assert_eq!(body["loop_state"], "turning");
}
