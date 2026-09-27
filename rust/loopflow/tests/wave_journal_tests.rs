//! The wave journal spine, end to end: a wave server's thread survives a
//! restart because the thread is a fold over the append-only journal. Also
//! covers the server's read surface over that spine (`/health`).

use std::path::Path;
use std::sync::Arc;

use loopflow::chat::turns::BodyProvenance;
use loopflow::chat::turns::ChatRole;
use loopflow::chat::types::{ConversationItem, Lifecycle};
use loopflow::controller::wave::journal::{
    fold_thread, journal_path, read_events, EventKind, Journal, MessageOp,
};
use loopflow::controller::wave::recovery::{self, FlowDisposition};
use loopflow::controller::wave::runtime::WaveRuntime;
use loopflow::controller::wave::server::{self, ResidentDoor};
use loopflow::controller::wave::state::LoopState;
use loopflow::controller::wave::wire::ResidentDelta;

#[test]
fn historical_skip_preserves_failure_and_queued_continuations_on_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let path = journal_path(tmp.path(), "ship");
    let saved = include_str!("../../../tests/fixtures/wave/queued-after-skip.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, saved).unwrap();

    // No source catalog exists here: the journal must retain the captured work.
    for _ in 0..2 {
        let error = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).unwrap_err();
        assert!(error.to_string().contains("lf wave recover ship"));
        let fold = fold_thread(&read_events(&path));
        let turns = &fold.turns;
        let failed = turns.iter().find(|turn| turn.id == "turn-3").unwrap();
        assert_eq!(failed.status, Lifecycle::Failed);
        assert_eq!(failed.text, "Partial repair evidence");
        let body = failed.body.as_ref().unwrap();
        assert_eq!(body.body_id, "failed-body");
        assert_eq!(body.session_id.as_deref(), Some("historical-session"));
        assert_eq!(
            body.termination_reason.as_deref(),
            Some("provider disconnected")
        );
        assert_eq!(turns.last().unwrap().text, "Keep later feedback too");
        assert_eq!(
            fold.pending_messages
                .iter()
                .map(|message| message.id.0.as_str())
                .collect::<Vec<_>>(),
            ["msg-1"],
            "only the historical failed claim returns to the governance queue"
        );
        assert!(fold.messages.keys().any(|id| id.0 == "msg-11"));

        let view = fold.playhead.unwrap();
        assert!(view.active.is_none());
        assert_eq!(
            view.stack
                .iter()
                .map(|frame| (frame.id.as_str(), frame.cursor, frame.iteration))
                .collect::<Vec<_>>(),
            [("root", 1, 2), ("nested", 1, 3)]
        );
        assert_eq!(view.stack[0].queue[0].id, "root-queued");
        assert_eq!(
            view.stack[1]
                .queue
                .iter()
                .map(|invocation| invocation.id.as_str())
                .collect::<Vec<_>>(),
            ["nested-first", "nested-second"]
        );
        let loopflow::engine::ConcreteStep::Skill(skill) = &view.stack[1].queue[0].steps[0] else {
            panic!("queued work retains its captured Skill")
        };
        assert_eq!(
            skill.skill.content.as_deref(),
            Some("Captured verify instructions")
        );
        assert_eq!(view.stack[1].cursor, 1);
        assert_eq!(view.stack[0].id, "root");
        assert!(std::fs::read_to_string(&path).unwrap().starts_with(saved));
    }
    assert_eq!(
        read_events(&path)
            .iter()
            .filter(|event| matches!(event.kind, EventKind::WaveFlowDisposition { .. }))
            .count(),
        1,
        "unresolved startup records one disposition across retries"
    );
}

fn install_history(repo: &Path, saved: &str) -> std::path::PathBuf {
    let path = journal_path(repo, "ship");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, saved).unwrap();
    path
}

#[test]
fn default_governance_cutover_is_recorded_once_before_new_work() {
    let tmp = tempfile::tempdir().unwrap();
    let original = include_str!("../../../tests/fixtures/wave/idle-default.jsonl");
    let path = install_history(tmp.path(), original);
    for _ in 0..2 {
        let runtime = open_wave(tmp.path());
        run_resident_turn(runtime, resident_turn_deltas());
    }
    let events = read_events(&path);
    assert!(fold_thread(&events).playhead.is_none());
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::WaveFlowDisposition { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::PlayheadChanged { .. }))
            .count(),
        1,
        "new governance never writes a playhead"
    );
    assert!(std::fs::read_to_string(path).unwrap().starts_with(original));
}

#[test]
fn cutover_never_infers_active_provider_death_from_listener_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let original = include_str!("../../../tests/fixtures/wave/active-default.jsonl");
    let path = install_history(tmp.path(), original);

    for _ in 0..2 {
        let error = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).unwrap_err();
        assert!(error.to_string().contains("liveness is unknown"));
        let report = recovery::inspect(tmp.path(), "ship").unwrap();
        let record = report.record.unwrap();
        assert!(
            recovery::cancel(tmp.path(), "ship", record.source_seq, "discard")
                .unwrap_err()
                .to_string()
                .contains("termination is unproven")
        );
        let events = read_events(&path);
        let fold = fold_thread(&events);
        assert_eq!(fold.open.len(), 1);
        assert_eq!(
            fold.playhead.unwrap().active.unwrap().body_id,
            "unsettled-body"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, EventKind::WaveFlowDisposition { .. }))
                .count(),
            1
        );
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .starts_with(original));
    }
}

#[test]
fn recovery_cli_cancels_only_the_reviewed_boundary() {
    let repo = loopflow_test_support::TestRepo::new();
    let path = journal_path(repo.path(), "ship");
    let saved = include_str!("../../../tests/fixtures/wave/queued-after-skip.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, saved).unwrap();
    let invoke = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(["wave", "recover", "ship"])
            .args(args)
            .current_dir(repo.path())
            .output()
            .unwrap()
    };
    let inspected = invoke(&[]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(report["record"]["source_seq"], 11);
    assert_eq!(
        report["source"]["kind"]["playhead"]["stack"][1]["queue"][0]["id"],
        "nested-first"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
    assert!(!invoke(&["--cancel", "10", "--reason", "stale selection"])
        .status
        .success());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);

    let cancelled = invoke(&["--cancel", "11", "--reason", "Replan as Tasks"]);
    assert!(
        cancelled.status.success(),
        "{}",
        String::from_utf8_lossy(&cancelled.stderr)
    );
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.starts_with(saved));
    assert!(invoke(&["--cancel", "11", "--reason", "Replan as Tasks"])
        .status
        .success());
    assert_eq!(std::fs::read_to_string(path).unwrap(), after);
}

/// One complete resident turn, as the loop emits it after a pass: an
/// item, the pass's reply text, then the finalized boundary.
fn resident_turn_deltas() -> Vec<ResidentDelta> {
    vec![
        ResidentDelta::TurnOpened {
            answers: vec![],
            body: None,
        },
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

#[tokio::test]
async fn restart_replays_thread_and_turn_ids_continue() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // First life: a user message and a real finalized turn.
    let before = {
        let rt = open_wave(tmp.path());
        rt.try_deliver(MessageOp::Message, "please build the feature".into(), None)
            .expect("journal write")
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
        .try_deliver(MessageOp::Message, "still there?".into(), None)
        .expect("journal write")
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
        use loopflow::controller::wave::journal::EventKind;
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

#[test]
fn restart_preserves_an_attempt_without_recorded_termination() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = open_wave(tmp.path());
    let body = BodyProvenance::for_wave(tmp.path());
    runtime.apply_resident_delta(ResidentDelta::TurnOpened {
        answers: vec![],
        body: Some(body.clone()),
    });
    drop(runtime);
    let path = journal_path(tmp.path(), "ship");
    let before = std::fs::read(&path).unwrap();
    for _ in 0..2 {
        let error = WaveRuntime::open("ship".into(), tmp.path().into()).unwrap_err();
        assert!(error.to_string().contains("unknown provider liveness"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(
            fold_thread(&read_events(&path)).open[0]
                .body
                .as_ref()
                .unwrap()
                .body_id,
            body.body_id
        );
    }
}

#[tokio::test]
async fn corrupt_trailing_line_is_tolerated_on_reboot() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let before = {
        let rt = open_wave(tmp.path());
        rt.try_deliver(MessageOp::Message, "kept message".into(), None)
            .expect("journal write")
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
    rt.try_deliver(MessageOp::Message, "after the crash".into(), None)
        .expect("journal write")
        .expect("user turn");
    assert_eq!(rt.thread_snapshot().len(), 2);
}

#[test]
fn legacy_playhead_cutover_preserves_original_source_until_explicit_cancellation() {
    for step in [
        serde_json::json!({"name": "research", "kind": "skill", "human": false}),
        serde_json::json!({"Xor": {"router": null, "flow_parents": [],
            "paths": {"gone": {"flow": "deleted-source", "skill": null, "description": "Old"}}
        }}),
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = journal_path(tmp.path(), "ship");
        std::fs::create_dir_all(path.parent().expect("journal parent"))
            .expect("create journal dir");
        let legacy = r#"{"v":1,"seq":1,"at":"2026-09-22T00:00:00Z","kind":{"type":"playhead_changed","event":{"kind":"definition_reset"},"playhead":{"stack":[{"id":"old-wave","flow":"wave","steps":[{"name":"research","kind":"skill","human":false}],"cursor":0,"iteration":3,"queue":[{"id":"queued-old","flow":"research","steps":[{"name":"research","kind":"skill","human":false}]}]}],"active":null}}}"#;
        let mut legacy: serde_json::Value = serde_json::from_str(legacy).unwrap();
        legacy["kind"]["playhead"]["stack"][0]["steps"] = serde_json::json!([step.clone()]);
        legacy["kind"]["playhead"]["stack"][0]["queue"][0]["steps"] = serde_json::json!([step]);
        let original = format!("{legacy}\n");
        std::fs::write(&path, &original).expect("write legacy journal");

        let error = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).unwrap_err();
        assert!(error.to_string().contains("lf wave recover ship"));
        let before = recovery::inspect(tmp.path(), "ship").unwrap();
        assert_eq!(before.source.as_ref(), Some(&legacy));
        assert!(matches!(
            before.record.unwrap().disposition,
            FlowDisposition::Unresolved { .. }
        ));

        recovery::cancel(tmp.path(), "ship", 1, "Move this work into Tasks").unwrap();
        let after_cancel = std::fs::read_to_string(&path).unwrap();
        recovery::cancel(tmp.path(), "ship", 1, "Move this work into Tasks").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), after_cancel);
        assert!(after_cancel.starts_with(&original));

        // A replacement never recompiles the old Flow or its queued source names.
        std::fs::create_dir_all(tmp.path().join(".lf/flows")).unwrap();
        std::fs::write(
            tmp.path().join(".lf/flows/wave.yaml"),
            "- source-does-not-exist\n",
        )
        .unwrap();
        for _ in 0..2 {
            let runtime = open_wave(tmp.path());
            run_resident_turn(runtime, resident_turn_deltas());
            assert!(fold_thread(&read_events(&path)).playhead.is_none());
        }
        assert_eq!(
            recovery::inspect(tmp.path(), "ship").unwrap().source,
            Some(legacy)
        );
    }
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
