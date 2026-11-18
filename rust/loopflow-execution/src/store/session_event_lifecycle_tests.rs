use crate::process::attachment;
use crate::store::sessions::history_tests::record_captured_start;
use serde_json::json;

#[test]
fn delayed_origin_preserves_pre_bind_assignment_through_a_conflicting_repeat() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let session = store.test_session(
        "conversation",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let process = loopflow_model::id::LfProcessId::new();
    let wave = loopflow_model::id::WaveId::new();
    let project = loopflow_storage::work::project::ProjectId::new();
    let task = loopflow_model::durable::TaskId::new();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&process],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
            [&wave],
        )
        .unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", rusqlite::params![project.as_str(),wave]).unwrap();
        conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
        conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,workspace_slug,created_at,updated_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','Proof','','',1,1)", rusqlite::params![task.as_str(),project.as_str()]).unwrap();
    }
    let attachment =
        attachment::claim_session_attachment(&store, &session.id, None, &process).unwrap();
    let before = store.session_turn_origin(&session.id, &attachment).unwrap();
    crate::store::sessions::bind_session(&store, &session.id, session.captured, &task).unwrap();
    let after = store.session_turn_origin(&session.id, &attachment).unwrap();
    store.record_session_turn_origin(&"thread".into(), "late", &before);
    let revisions = store.revisions().unwrap();
    store.record_session_turn_origin(&"thread".into(), "late", &before);
    assert_eq!(store.revisions().unwrap(), revisions);
    // A conflicting repeat is a recording gap; the turn keeps its origin.
    store.record_session_turn_origin(&"thread".into(), "late", &after);
    store.record_session_turn_origin(&"thread".into(), "new", &after);
    let turns = store.session_turns(&session.id).unwrap();
    let late = turns.iter().find(|turn| turn.turn == "late").unwrap();
    let new = turns.iter().find(|turn| turn.turn == "new").unwrap();
    assert_eq!(late.task_id, None);
    assert_eq!(late.wave_id, None);
    assert_eq!(new.task_id.as_deref(), Some(task.as_str()));
    assert_eq!(new.wave_id.as_deref(), Some(wave.as_str()));
    store.move_task(&task, Some(&project), None).unwrap();
    let detached = store.session_turn_origin(&session.id, &attachment).unwrap();
    store.record_session_turn_origin(&"thread".into(), "dispatched-before-move", &after);
    store.record_session_turn_origin(&"thread".into(), "dispatched-after-move", &detached);
    let turns = store.session_turns(&session.id).unwrap();
    let before_move = turns
        .iter()
        .find(|turn| turn.turn == "dispatched-before-move")
        .unwrap();
    let after_move = turns
        .iter()
        .find(|turn| turn.turn == "dispatched-after-move")
        .unwrap();
    assert_eq!(before_move.wave_id.as_deref(), Some(wave.as_str()));
    assert_eq!(after_move.wave_id, None);
    assert_eq!(after_move.task_id.as_deref(), Some(task.as_str()));
}

#[test]
#[cfg(unix)]
fn session_exit_closes_its_agent_process() {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let session = store.test_session(
        "conversation",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let first = loopflow_model::id::LfProcessId::new();
    {
        let conn = store.conn.lock().unwrap();
        // The AgentProcess inherits this trace; the inventory reads it typed.
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?2,1)",
            rusqlite::params![first, loopflow_model::id::TraceId::new()],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET provider='codex',interactive=0 WHERE id=?1",
            [&session.id],
        )
        .unwrap();
    }
    let original = attachment::claim_session_attachment(&store, &session.id, None, &first).unwrap();
    let mut child = Command::new("sleep")
        .arg("30")
        .process_group(0)
        .spawn()
        .unwrap();
    let started = crate::process::os_process::process_started_at(child.id())
        .unwrap()
        .unwrap();
    store
        .record_agent_process_identity(&session.id, &original, child.id(), started)
        .unwrap();
    // An AgentProcess whose socket has gone must still be reaped on exit.
    store
        .record_session_connection(
            &session.id,
            &original,
            home.path().join("agent.sock").to_str().unwrap(),
            &"saved-thread".into(),
        )
        .unwrap();
    crate::process::finish_session_attachment(&store, &session.id, &original, "completed").unwrap();
    // Close leaves reaping to the parent; an unreaped zombie is dead.
    assert_eq!(
        crate::process::os_process::process_identity_evidence(child.id(), started),
        crate::store::ProcessIdentityEvidence::Dead
    );
    let _ = child.wait();
    assert!(attachment::session_connection(&store, &session.id)
        .unwrap()
        .is_none());
    assert_eq!(
        store.session_thread(&session.id).unwrap(),
        Some("saved-thread".into())
    );
    assert!(store
        .session_attachment(&session.id)
        .unwrap()
        .unwrap()
        .lf_process_id
        .is_none());
}

#[test]
fn session_exit_retires_orphans_and_headless_runs_but_preserves_primary_and_task_conversations() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let wave = loopflow_model::id::WaveId::new();
    let project = loopflow_storage::work::project::ProjectId::new();
    let task = loopflow_storage::work::task::TaskId::new();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
            [&wave],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project-proof',1)",
            rusqlite::params![project.as_str(), wave],
        ).unwrap();
        conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
        conn.execute(
            "INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue-proof','PROOF-1',1)",
            rusqlite::params![task.as_str(), project.as_str()],
        ).unwrap();
    }
    for (id, primary, outcome, retired) in [
        ("orphan", None, "interrupted", true),
        ("primary", Some("repository"), "interrupted", false),
        ("task", None, "interrupted", false),
        ("task-run", None, "interrupted", true),
        ("task-failed-run", None, "failed", true),
    ] {
        let session = store.test_session(id, &loopflow_storage::session_record::new_artifact_key());
        let process = loopflow_model::id::LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_sessions SET primary_scope=?2 WHERE id=?1",
                rusqlite::params![id, primary],
            )
            .unwrap();
            if id.starts_with("task") {
                conn.execute(
                    "UPDATE agent_sessions SET task_id=?2,wave_id=?3,interactive=?4 WHERE id=?1",
                    rusqlite::params![id, task.as_str(), wave, id == "task"],
                )
                .unwrap();
            }
        }
        let attachment = attachment::claim_session_attachment(&store, id, None, &process).unwrap();
        attachment::finish_session_attachment(&store, id, &attachment, outcome, None, || Ok(false))
            .unwrap();
        let saved = store.session(id).unwrap().unwrap();
        assert_eq!(saved.completed_at.is_some(), retired);
        assert_eq!(saved.captured, session.captured);
        // An exit with nowhere to go writes no history: its outcome is the
        // Process's, and it completes no turn.
        let history = store.session_history(id, 0, 100).unwrap();
        assert!(!history
            .iter()
            .any(|event| event.payload["type"] == "attachment_exit"));
        assert!(store.session_turns(id).unwrap().is_empty());
    }
    assert!(!crate::store::session_projection::session_summaries(
        &store,
        &loopflow_storage::session::SessionFilter::default(),
        0
    )
    .unwrap()
    .iter()
    .any(|session| session.id == "orphan"));

    // Another turn on the finished run reopens it until that turn exits.
    let process = loopflow_model::id::LfProcessId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',2)",
            [process.as_str()],
        )
        .unwrap();
    let expected = store.session_attachment("task-run").unwrap();
    let turn =
        attachment::claim_session_attachment(&store, "task-run", expected.as_ref(), &process)
            .unwrap();
    assert!(store
        .session("task-run")
        .unwrap()
        .unwrap()
        .completed_at
        .is_none());
    attachment::finish_session_attachment(&store, "task-run", &turn, "completed", None, || {
        Ok(true)
    })
    .unwrap();
    assert!(store
        .session("task-run")
        .unwrap()
        .unwrap()
        .completed_at
        .is_some());
}

#[test]
fn completed_turn_does_not_complete_the_conversation() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let session = store.test_session(
        "unfinished",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    store.record_turn_start(&session.id, &"thread".into(), "turn", 1);
    store.record_turn_completion(
        &session.id,
        &"thread".into(),
        "turn",
        2,
        &json!({"status": "completed"}),
    );
    for _ in 0..2 {
        let rows = crate::store::session_projection::session_summaries(
            &store,
            &loopflow_storage::session::SessionFilter::default(),
            0,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, session.id.as_str());
        assert!(rows[0].completed_at.is_none());
    }
}

#[test]
fn stopped_turn_and_stale_attachment_cannot_retire_a_resumed_conversation() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let session = store.test_session(
        "conversation",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let first = loopflow_model::id::LfProcessId::new();
    let second = loopflow_model::id::LfProcessId::new();
    for process in [&first, &second] {
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
    }
    let original = attachment::claim_session_attachment(&store, &session.id, None, &first).unwrap();
    record_captured_start(&store, &session.id, "turn");
    store.record_turn_completion(
        &session.id,
        &"thread".into(),
        "turn",
        2,
        &json!({"status":"interrupted"}),
    );
    assert!(store
        .session(&session.id)
        .unwrap()
        .unwrap()
        .completed_at
        .is_none());
    let resumed =
        attachment::claim_session_attachment(&store, &session.id, Some(&original), &second)
            .unwrap();
    assert!(attachment::finish_session_attachment(
        &store,
        &session.id,
        &original,
        "interrupted",
        None,
        || Ok(false)
    )
    .is_err());
    assert!(store
        .session(&session.id)
        .unwrap()
        .unwrap()
        .completed_at
        .is_none());
    assert_eq!(
        store.session_attachment(&session.id).unwrap().unwrap(),
        resumed
    );
    attachment::finish_session_attachment(&store, &session.id, &resumed, "completed", None, || {
        Ok(false)
    })
    .unwrap();
    assert!(store
        .session(&session.id)
        .unwrap()
        .unwrap()
        .completed_at
        .is_some());
}

#[test]
fn recovered_completion_keeps_missing_start_and_usage_and_its_first_answer() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session("conversation", "run_00000000000000000000000000000001");
    let completed = json!({"status":"completed", "error":null, "started_at":10, "completed_at":20, "duration_ms":10000});
    let record = |payload: &serde_json::Value| {
        store.record_turn_completion("conversation", &"thread".into(), "turn", 20, payload)
    };
    record(&completed);
    record(&completed);
    // A conflicting receipt is a recording gap; the first answer stays.
    record(&json!({"status":"failed"}));
    let turns = store.session_turns("conversation").unwrap();
    assert_eq!(turns.len(), 1);
    let turn = &turns[0];
    assert_eq!(
        (turn.started_at, &turn.usage),
        (None, &None),
        "recovery cannot invent start or usage receipts"
    );
    assert_eq!(
        (turn.ended_at, turn.status.as_deref(), turn.duration_ms),
        (Some(20), Some("completed"), Some(10000))
    );
    assert_eq!(turn.agent_process_id, None);
    assert_eq!(turn.lf_process_id, None);

    let first = loopflow_model::id::LfProcessId::new();
    let second = loopflow_model::id::LfProcessId::new();
    for process in [&first, &second] {
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
    }
    let original =
        attachment::claim_session_attachment(&store, "conversation", None, &first).unwrap();
    store
        .record_session_connection(
            "conversation",
            &original,
            "/original.sock",
            &"thread".into(),
        )
        .unwrap();
    store
        .record_agent_process_identity("conversation", &original, 12345, 12)
        .unwrap();
    store.record_session_turn_origin(
        &"thread".into(),
        "later",
        &store
            .session_turn_origin("conversation", &original)
            .unwrap(),
    );
    store
        .record_agent_process_exit("conversation", &original, true)
        .unwrap();
    let replacement =
        attachment::claim_session_attachment(&store, "conversation", Some(&original), &second)
            .unwrap();
    assert_ne!(replacement.agent_process_id, original.agent_process_id);
    assert!(attachment::session_connection(&store, "conversation")
        .unwrap()
        .is_none());
    assert!(store
        .agent_process_identity("conversation")
        .unwrap()
        .is_none());
    assert_eq!(
        store.session_thread("conversation").unwrap(),
        Some("thread".into())
    );
    assert!(store
        .record_session_connection(
            "conversation",
            &original,
            "/stale.sock",
            &"wrong-thread".into()
        )
        .is_err());
    assert!(store
        .record_agent_process_identity("conversation", &original, 12346, 13)
        .is_err());
    for turn in ["later", "unobserved"] {
        store.record_turn_usage(
            "conversation",
            &"thread".into(),
            turn,
            30,
            &json!({"total":{"inputTokens":40}}),
        );
    }
    let turns = store.session_turns("conversation").unwrap();
    let turn = |id: &str| turns.iter().find(|turn| turn.turn == id).unwrap();
    assert_eq!(
        turn("later").agent_process_id.as_deref(),
        Some(original.agent_process_id.as_str())
    );
    assert_eq!(turn("later").lf_process_id.as_deref(), Some(first.as_str()));
    assert_eq!(turn("unobserved").agent_process_id, None);
    assert_eq!(turn("unobserved").lf_process_id, None);
}
