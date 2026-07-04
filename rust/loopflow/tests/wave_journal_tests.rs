//! The wave journal spine, end to end: a wave server's thread survives a
//! restart because the thread is a fold over the append-only journal.

use std::path::Path;
use std::sync::Arc;

use loopflow::lfd::conversations::turns::ChatRole;
use loopflow::lfd::conversations::types::Lifecycle;
use loopflow::lfd::wave::journal::{fold_thread, journal_path, Journal};
use loopflow::lfd::wave::runtime::{TurnSink, WaveRuntime};
use loopflow::lfd::wave::server;
use loopflow::lfd::wave::state::MindState;
use loopflow::lfd::wave::subagent::{run_subagent, SubagentSpec};

/// Canned codex `exec --json` output replayed via `printf` — the same
/// replay-binary fixture the subagent unit tests use.
fn replay_spec(lines: &[&str]) -> SubagentSpec {
    SubagentSpec {
        label: "replay".to_string(),
        program: "printf".to_string(),
        args: vec!["%s\\n".to_string(), lines.join("\n")],
        cwd: std::env::temp_dir(),
    }
}

const CODEX_FIXTURE: &[&str] = &[
    r#"{"type":"item.started","item":{"type":"command_execution","command":"cargo test"}}"#,
    r#"{"type":"item.completed","item":{"type":"agent_message","text":"Implemented the feature."}}"#,
    r#"{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":5}}"#,
];

/// Run one replay pass through the production pipeline (parser → TurnBuilder
/// deltas → TurnSink → journal + thread).
async fn run_replay_pass(runtime: Arc<WaveRuntime>, lines: &[&str]) {
    let mut sink = TurnSink::new(runtime);
    run_subagent(&replay_spec(lines), |delta| sink.on_delta(delta))
        .await
        .expect("run replay pass");
}

fn turn_seq(id: &str) -> u64 {
    id.strip_prefix("turn-")
        .and_then(|n| n.parse().ok())
        .expect("turn id from journal seq")
}

fn open_wave(repo: &Path) -> Arc<WaveRuntime> {
    WaveRuntime::open("ship".into(), repo.to_path_buf())
        .expect("open runtime")
        .0
}

#[tokio::test]
async fn restart_replays_thread_and_turn_ids_continue() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // First life: a user message and a real finalized turn.
    let before = {
        let rt = open_wave(tmp.path());
        rt.deliver_user_message("please build the feature".into());
        run_replay_pass(rt.clone(), CODEX_FIXTURE).await;
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
    let next = rt.deliver_user_message("still there?".into());
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
    run_replay_pass(rt.clone(), CODEX_FIXTURE).await;
    let live = rt.thread_snapshot();

    // Independent fold of the raw journal — no runtime involved.
    let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen journal");
    let fold = fold_thread(&events);
    assert!(fold.open.is_empty());
    assert_eq!(fold.turns, live, "fold(journal) == live thread");

    // And both match what the old in-memory path produced for this fixture.
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
    // A pass that dies mid-turn: content but no terminating result. The
    // subagent driver finalizes it Failed — now also simulate the server
    // crashing before that, by writing the started/item events only.
    {
        let (mut journal, _) = Journal::open(&journal_path(tmp.path(), "ship")).expect("open");
        use loopflow::lfd::conversations::types::ConversationItem;
        use loopflow::lfd::wave::journal::EventKind;
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
        rt.deliver_user_message("kept message".into());
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
    rt.deliver_user_message("after the crash".into());
    assert_eq!(rt.thread_snapshot().len(), 2);
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

#[tokio::test]
async fn health_reports_the_mind_state() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let rt = open_wave(tmp.path());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = server::router(rt.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    let body = reqwest::get(format!("http://{addr}/health"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("\"status\":\"idle\""));

    rt.transition(
        MindState::Turning {
            turn_id: "turn-1".into(),
        },
        "test turn",
    );
    let body = reqwest::get(format!("http://{addr}/health"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("\"status\":\"turning\""));
}
