use loopflow_storage::session::{GraphProcessSummaryState, SessionFilter};
use rusqlite::params;
use serde_json::json;

#[test]
fn session_inventory_distinguishes_primary_sessions_from_unassigned_tasks() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session(
        "primary",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    store.test_session(
        "other",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let repository = loopflow_model::durable::RepositoryId::new();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO repository_plans(id,repo,selected) VALUES(?1,'/repo',1)",
            [repository.as_str()],
        )
        .unwrap();
        for primary in [Some("primary"), None] {
            let task = loopflow_model::durable::TaskId::new();
            conn.execute(
                "INSERT INTO tasks(repository_id,id,created_at,issue_identifier)
                 VALUES(?1,?2,1,?2)",
                params![repository.as_str(), task.as_str()],
            )
            .unwrap();
            if let Some(primary) = primary {
                conn.execute(
                    "UPDATE agent_sessions SET task_id=?1,wave_id=(SELECT p.wave_id FROM tasks t LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?1) WHERE id=?2",
                    params![task.as_str(), primary],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO task_primary_sessions(task_id,session_id) VALUES(?1,?2)",
                    params![task.as_str(), primary],
                )
                .unwrap();
            }
        }
    }
    let rows =
        crate::store::session_projection::session_summaries(&store, &SessionFilter::default(), 0)
            .unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(
            row.primary_scope.as_deref() == Some("task"),
            row.id.as_str() == "primary"
        );
    }
}

#[test]
fn input_replacement_retains_workspace_and_task_membership() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let session = store.test_session(
        "moving",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let wave = loopflow_model::id::WaveId::new();
    let project = loopflow_storage::work::project::ProjectId::new();
    let task = loopflow_model::durable::TaskId::new();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
            [&wave],
        )
        .unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(), wave]).unwrap();
        conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
        conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,workspace_slug,updated_at,created_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','Proof','','proof',1,1)", params![task.as_str(), project.as_str()]).unwrap();
        conn.execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
            params![task.as_str(), session.cwd.to_str().unwrap()],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET task_id=?1,wave_id=?2 WHERE id=?3",
            params![task.as_str(), wave, session.id],
        )
        .unwrap();
    }
    let session = store.session(&session.id).unwrap().unwrap();
    assert_eq!(
        crate::store::task_work::task_work(&store, &task)
            .unwrap()
            .sessions
            .len(),
        1
    );
    let exclusion = crate::store::admission::lock_checkout(&store, &session.cwd).unwrap();
    let mut next = session.clone();
    next.cwd = home.path().join("destination");
    next.artifact_key = Some(loopflow_storage::session_record::new_artifact_key());
    assert!(crate::store::sessions::replace_session_input(
        &store,
        session.captured,
        next.clone(),
        None
    )
    .is_err());
    let retained = crate::store::task_work::task_work(&store, &task)
        .unwrap()
        .sessions;
    drop(exclusion);
    assert_eq!(
        retained.len(),
        1,
        "input replacement removed Task work while its source checkout was excluded"
    );
    let replacement =
        crate::store::sessions::replace_session_input(&store, session.captured, next, None)
            .unwrap();
    assert_eq!(replacement.cwd, session.cwd);
    assert_eq!(replacement.task_id, Some(task.clone()));
    assert_eq!(
        crate::store::task_work::task_work(&store, &task)
            .unwrap()
            .sessions
            .len(),
        1
    );
    assert_ne!(replacement.captured, session.captured);
}

#[test]
fn retained_sessions_without_repo_use_their_recorded_wave() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session(
        "retained",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let wave = loopflow_model::id::WaveId::new();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'Product','/repo',1)",
            [wave.as_str()],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET wave_id=?1,repo=NULL WHERE id='retained'",
            [wave.as_str()],
        )
        .unwrap();
    }
    let filter = SessionFilter {
        repo: Some("/repo".into()),
        ..SessionFilter::default()
    };
    assert_eq!(
        crate::store::session_projection::session_summaries(&store, &filter, 0).unwrap()[0].id,
        "retained"
    );
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET repo='/other' WHERE id='retained'",
            [],
        )
        .unwrap();
    assert!(
        crate::store::session_projection::session_summaries(&store, &filter, 0)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn retrying_retired_input_preserves_completion_and_rejects_stale_snapshots() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let first = store.test_session(
        "retired",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let mut next = first.clone();
    next.artifact_key = Some(loopflow_storage::session_record::new_artifact_key());
    next.input_published = false;
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET completed_at=10 WHERE id='retired'",
            [],
        )
        .unwrap();
    let before = store.session_history("retired", 0, 100).unwrap();
    assert!(crate::store::sessions::replace_session_input(
        &store,
        first.captured,
        next.clone(),
        None
    )
    .is_err());
    assert_eq!(store.session_history("retired", 0, 100).unwrap(), before);

    next.completed_at = Some(10);
    let next =
        crate::store::sessions::replace_session_input(&store, first.captured, next, None).unwrap();
    store.publish_capture(&next.id, next.captured).unwrap();
    let retained = store.session("retired").unwrap().unwrap();
    assert_eq!(retained.completed_at, Some(10));
    assert!(retained.input_published);
    assert_ne!(retained.captured, first.captured);
}

#[test]
fn captured_reservation_survives_interruption_and_fences_replacement() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("store.db");
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let old_key = "run_00000000000000000000000000000001";
    let first = store.test_session("conversation", old_key);
    let mut next = first.clone();
    next.artifact_key = Some(loopflow_storage::session_record::new_artifact_key());
    next.input_published = false;
    let next =
        crate::store::sessions::replace_session_input(&store, first.captured, next, None).unwrap();
    assert_ne!(next.captured, first.captured);
    drop(store);
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let retained = store.session("conversation").unwrap().unwrap();
    assert_eq!(retained, next);
    assert!(!retained.input_published);
    assert_eq!(store.captured_sequence(old_key).unwrap(), first.captured);
    assert_eq!(
        store.session_for_artifact(old_key).unwrap().unwrap().id,
        first.id
    );
    assert!(store.publish_capture(&first.id, first.captured).is_err());
    assert!(crate::store::sessions::replace_session_input(
        &store,
        first.captured,
        first.clone(),
        None
    )
    .is_err());
    store
        .publish_capture(&retained.id, retained.captured)
        .unwrap();
    assert!(store
        .publish_capture(&retained.id, retained.captured)
        .is_err());
    assert_eq!(store.session_inputs("conversation").unwrap().len(), 2);
    assert!(
        store
            .session("conversation")
            .unwrap()
            .unwrap()
            .input_published
    );
}

#[test]
fn session_metadata_reads_its_graph_from_the_process_that_captured_its_input() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let input = loopflow_storage::session_record::new_artifact_key();
    let graph_process = store.test_graph("retained", "/unavailable", &[("implement", None)], None);
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,
            interactive,input_published,cwd,request)
            VALUES('session','Session','human',1,1,1,'/unavailable',?1)",
            params!["large request".repeat(1000)],
        )
        .unwrap();
        // The step Process captured this conversation's input.
        conn.execute(
            "INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload)
             VALUES('session','captured',?1,(SELECT id FROM processes WHERE parent_lf_process_id=?2),1,
                json_object('artifact_key',?1))",
            params![input, graph_process],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET current_capture=?1 WHERE id='session'",
            [conn.last_insert_rowid()],
        )
        .unwrap();
        store.execute_history(
"INSERT INTO session_history(session_id,receipt_key,observed_at,payload,captured_event)
            VALUES('session',?1,1,'not JSON',(SELECT current_capture FROM agent_sessions WHERE id='session'))",
            [format!("{input}:events.jsonl:0")],
        )
        .unwrap();
    }
    // Listing reads the Graph from Processes and survives unreadable detail.
    let rows =
        crate::store::session_projection::session_summaries(&store, &SessionFilter::default(), 0)
            .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].graph_lf_process_id.as_deref(),
        Some(graph_process.as_str())
    );
    let graph = rows[0].graph.as_ref().unwrap();
    assert_eq!(
        (graph.name.as_str(), graph.state),
        ("retained", GraphProcessSummaryState::Current)
    );
    assert!(rows[0].graph_step_latest);
    assert_eq!(
        store
            .session("session")
            .unwrap()
            .unwrap()
            .graph_lf_process_id
            .as_deref(),
        Some(graph_process.as_str())
    );
    assert!(
        store.input_events(&input).is_err(),
        "Exact history still reports corrupt payload"
    );
    let pid = std::process::id();
    let started = crate::process::os_process::process_started_at(pid)
        .unwrap()
        .unwrap();
    let graph_state = || {
        crate::store::session_projection::session_summary(&store, "session", 0)
            .unwrap()
            .unwrap()
            .graph
            .unwrap()
            .state
    };
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE graph_processes SET pid=?2,os_started_at=?3 WHERE lf_process_id=?1",
            params![graph_process, pid, started + 60],
        )
        .unwrap();
    // A Graph run whose OS process is gone without writing its end stopped.
    assert_eq!(graph_state(), GraphProcessSummaryState::Stopped);
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE graph_processes SET os_started_at=?2 WHERE lf_process_id=?1",
            params![graph_process, started],
        )
        .unwrap();
    assert_eq!(graph_state(), GraphProcessSummaryState::Current);
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE graph_processes SET ended_at=2,exit_code=0,pid=NULL,os_started_at=NULL
             WHERE lf_process_id=?1",
            params![graph_process],
        )
        .unwrap();
    assert_eq!(graph_state(), GraphProcessSummaryState::Completed);
}

#[test]
fn session_metadata_receipts_remain_idempotent_and_conflicts_do_not_relabel() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let input = loopflow_storage::session_record::new_artifact_key();
    store.test_session("imported", &input);
    let session = store.session("imported").unwrap().unwrap();
    let mut observation = loopflow_storage::session::SessionObservation {
        artifact_key: input.clone(),
        source: "manifest.json".into(),
        observed_at: 1,
        task_id: None,
        wave_id: None,
        payload: json!({"input_id":input.as_str(),"source":"manifest.json","evidence":{
            "artifact_key":input.as_str(),"schema_version":1,"flow":{"kind":"independent"}}}),
    };
    assert!(
        !crate::store::session_projection::session_summary(&store, "imported", 0)
            .unwrap()
            .unwrap()
            .independent
    );
    for _ in 0..2 {
        store
            .retain_session_observation(&session, &observation)
            .unwrap();
    }
    assert!(
        crate::store::session_projection::session_summary(&store, "imported", 0)
            .unwrap()
            .unwrap()
            .independent
    );
    assert_eq!(store.session_history("imported", 0, 0).unwrap().len(), 1);
    // A conflicting receipt is a recording gap: the first one stays.
    observation.payload["evidence"]["flow"] = serde_json::Value::Null;
    store
        .retain_session_observation(&session, &observation)
        .unwrap();
    assert_eq!(
        store.session_history("imported", 0, 0).unwrap()[0].payload["evidence"]["flow"]["kind"],
        "independent"
    );
    assert!(
        crate::store::session_projection::session_summary(&store, "imported", 0)
            .unwrap()
            .unwrap()
            .independent
    );
}

#[test]
fn session_metadata_filters_and_preserves_explicit_membership() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    {
        let conn = store.conn.lock().unwrap();
        for (id, title, interactive, repo, membership) in [
            ("a", "Alpha%", true, "/repo", Some(())),
            ("b", "Beta", true, "/repo", None),
            ("headless", "Headless", false, "/repo", None),
            ("closed", "Closed", true, "/repo", None),
            ("closed-headless", "Closed headless", false, "/repo", None),
            ("foreign", "Foreign", true, "/other", None),
            ("later", "Zulu", true, "/repo", None),
        ] {
            let input = loopflow_storage::session_record::new_artifact_key();
            conn.execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,
                interactive,input_published,cwd,repo)
                VALUES(?1,?2,'human',1,?3,1,'/unavailable',?4)",
                params![id, title, interactive, repo],
            )
            .unwrap();
            loopflow_storage::sqlite::sessions::test_capture(&conn, id, &input);
            if membership.is_some() {
                conn.execute(
                    "UPDATE session_events SET independent=1
                     WHERE seq=(SELECT current_capture FROM agent_sessions WHERE id=?1)",
                    [id],
                )
                .unwrap();
            }
        }
    }
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET completed_at=2 WHERE id IN ('closed','closed-headless')",
            [],
        )
        .unwrap();
    let mut filter = SessionFilter {
        repo: Some("/repo".into()),
        limit: 2,
        ..SessionFilter::default()
    };
    let rows = crate::store::session_projection::session_summaries(&store, &filter, 0).unwrap();
    assert_eq!(
        rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert!(rows[0].independent);
    assert!(!rows[1].independent);
    filter.search = Some("%".into());
    assert_eq!(
        crate::store::session_projection::session_summaries(&store, &filter, 0)
            .unwrap()
            .len(),
        1,
        "Contains search is literal"
    );
    filter.search = Some("ALPHA".into());
    assert_eq!(
        crate::store::session_projection::session_summaries(&store, &filter, 0).unwrap()[0].id,
        "a"
    );
    filter.search = None;
    filter.headless = true;
    filter.limit = 0;
    assert_eq!(
        crate::store::session_projection::session_summaries(&store, &filter, 0)
            .unwrap()
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "headless", "later"]
    );
    filter.headless = false;
    filter.limit = 1;
    filter.after = Some(String::new());
    let first = crate::store::session_projection::session_summaries(&store, &filter, 0).unwrap();
    assert_eq!(first[0].id, "a");
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET title='000 renamed' WHERE id='b'",
            [],
        )
        .unwrap();
    filter.after = Some(first[0].id.to_string());
    let second = crate::store::session_projection::session_summaries(&store, &filter, 0).unwrap();
    assert_eq!(second[0].id, "b");
    assert_eq!(second[0].title, "000 renamed");
    filter.after = Some(String::new());
    filter.limit = 2;
    assert_eq!(
        crate::store::session_projection::session_summaries(&store, &filter, 0)
            .unwrap()
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"],
        "renaming must not reorder the ID page used to build its cursor"
    );
}
