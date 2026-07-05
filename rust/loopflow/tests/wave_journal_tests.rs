//! The wave journal spine, end to end: a wave server's thread survives a
//! restart because the thread is a fold over the append-only journal. Also
//! covers the server's read surface over that spine (`/health`).

use std::path::Path;
use std::sync::Arc;

use loopflow::conversation::turns::ChatRole;
use loopflow::conversation::types::{ConversationEvent, ConversationItem, Lifecycle, TurnUsage};
use loopflow::wave::journal::{fold_thread, journal_path, Journal, MessageOp};
use loopflow::wave::mind::EventAdapter;
use loopflow::wave::runtime::WaveRuntime;
use loopflow::wave::server::{self, ResidentDoor};
use loopflow::wave::state::MindState;

/// One complete harness turn, as the codex driver would emit it: a command
/// item, the final agent message, the turn completion, then trailing usage.
fn codex_turn_events() -> Vec<ConversationEvent> {
    vec![
        ConversationEvent::TurnStarted {
            turn_id: "vt-1".into(),
        },
        ConversationEvent::ItemCompleted {
            turn_id: "vt-1".into(),
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
        ConversationEvent::ItemCompleted {
            turn_id: "vt-1".into(),
            item: ConversationItem::Message {
                id: "item-1".into(),
                text: "Implemented the feature.".into(),
                phase: None,
            },
        },
        ConversationEvent::TurnCompleted {
            turn_id: "vt-1".into(),
            status: Lifecycle::Completed,
        },
        ConversationEvent::TurnUsage {
            turn_id: "vt-1".into(),
            usage: TurnUsage {
                input_tokens: 10,
                output_tokens: 5,
                reasoning_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                model: None,
                cost_usd: None,
            },
        },
    ]
}

/// Run one harness turn through the production pipeline (EventAdapter →
/// resident wire deltas → the listener's fold), as the resident door would.
fn run_harness_turn(runtime: Arc<WaveRuntime>, events: &[ConversationEvent]) {
    let mut adapter = EventAdapter::new();
    for event in events {
        for delta in adapter.feed(event) {
            runtime.apply_resident_delta(delta);
        }
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

#[tokio::test]
async fn restart_replays_thread_and_turn_ids_continue() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // First life: a user message and a real finalized turn.
    let before = {
        let rt = open_wave(tmp.path());
        rt.deliver_user_message("please build the feature".into(), MessageOp::Message);
        run_harness_turn(rt.clone(), &codex_turn_events());
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
    assert_eq!(rt.mind_state(), MindState::Idle);

    // And new turn ids continue the journal's seq domain monotonically.
    let max_before = before.iter().map(|t| turn_seq(&t.id)).max().unwrap();
    let next = rt.deliver_user_message("still there?".into(), MessageOp::Message);
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
    run_harness_turn(rt.clone(), &codex_turn_events());
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
        });
        journal.append(|_| EventKind::TurnItem {
            turn_id: "turn-1".into(),
            item: ConversationItem::Message {
                id: "text-0".into(),
                text: "half a thought".into(),
                phase: None,
            },
        });
        journal.append(|_| EventKind::MindState {
            from: MindState::Idle,
            to: MindState::Turning {
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
    assert_eq!(rt.mind_state(), MindState::Idle, "janitor settled the mind");
}

#[tokio::test]
async fn corrupt_trailing_line_is_tolerated_on_reboot() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let before = {
        let rt = open_wave(tmp.path());
        rt.deliver_user_message("kept message".into(), MessageOp::Message);
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
    rt.deliver_user_message("after the crash".into(), MessageOp::Message);
    assert_eq!(rt.thread_snapshot().len(), 2);
}

#[tokio::test]
async fn thread_started_survives_a_restart() {
    let tmp = tempfile::tempdir().expect("tempdir");
    {
        let rt = open_wave(tmp.path());
        assert_eq!(rt.last_thread_id(), None);
        rt.journal_thread_started("codex", "thread-abc");
    }
    // The resume handle is a fold of the journal, like everything else.
    let rt = open_wave(tmp.path());
    assert_eq!(rt.last_thread_id().as_deref(), Some("thread-abc"));
}

#[tokio::test]
async fn illegal_mind_transition_is_refused() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let rt = open_wave(tmp.path());

    assert!(!rt.transition(
        MindState::Interrupting {
            turn_id: "turn-1".into()
        },
        "nothing to interrupt"
    ));
    assert_eq!(rt.mind_state(), MindState::Idle, "state untouched");
    // Refused moves leave no trace in the journal.
    let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("open journal");
    assert!(events.is_empty());
}

/// `/health` splits channel liveness (`status`, always `serving`) from the
/// resident's condition (`mind`: null while no resident was ever spawned or
/// attached, then the mind-state name).
#[tokio::test]
async fn health_reports_channel_liveness_and_the_mind_state() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let rt = open_wave(tmp.path());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = server::router(rt.clone(), ResidentDoor::new("test-token"), None, None);
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
    assert!(body["mind"].is_null(), "no resident yet: a dormant channel");

    rt.set_resident_expected();
    let body: serde_json::Value = reqwest::get(format!("http://{addr}/health"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["mind"], "idle");

    rt.transition(
        MindState::Turning {
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
    assert_eq!(body["status"], "serving", "channel liveness is constant");
    assert_eq!(body["mind"], "turning");
}
