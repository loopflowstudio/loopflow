//! Application admission, binding and checkout exclusion.
use crate::process::attachment;

use crate::session::WorkSource;
use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_model::id::WaveId;
use loopflow_storage::planning::LinearProjectId;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::test_fixture::{unpublished_conversation, unregistered_task};
use std::sync::{Arc, Barrier};
use std::thread;
fn store_with_task() -> (tempfile::TempDir, SqliteStore, TaskId) {
    let (dir, store, task, pr) = loopflow_storage::test_fixture::task();
    let me = crate::store::ownership::acquire(
        &store,
        store
            .new_task_scope(&task.repository_id, task.project_id.as_ref())
            .unwrap(),
    )
    .unwrap();
    store.seed_task(&task, &pr, Some(&me)).unwrap();
    (dir, store, task.id)
}

#[test]
fn a_task_launches_in_any_project_of_its_wave() {
    let (_dir, store, task_id) = store_with_task();
    let task = store.task(&task_id).unwrap().unwrap();
    let mut newer = store
        .project(task.project_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    newer.id = ProjectId::new();
    newer.tracker_id = Some(LinearProjectId::new("218967b6-a760-4b7c-9a46-11d9d61a42c2").unwrap());
    newer.name = "A newer started Project".into();
    newer.created_at += time::Duration::days(1);
    store.insert_project(&newer).unwrap();
    // New Tasks now join the newer Project; this one still runs where it is,
    // whatever its Project's status, and launching claims nothing.
    crate::store::ownership::unassign_task(&store, &task_id).unwrap();
    assert_eq!(
        store
            .new_task_project(task.wave_id.as_ref().unwrap())
            .unwrap()
            .unwrap(),
        newer.id
    );
    for status in [
        "started",
        "backlog",
        "planned",
        "paused",
        "completed",
        "canceled",
    ] {
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE projects SET status=?2 WHERE id=?1",
                rusqlite::params![task.project_id.as_ref().unwrap().as_str(), status],
            )
            .unwrap();
        store.require_task_launch(&task_id).unwrap();
    }
    assert!(!store.task_owned(&task_id).unwrap());
}

#[test]
fn descendant_admission_excludes_registration_of_a_missing_root() {
    let (dir, store, existing) = store_with_task();
    let (task, pr) = unregistered_task(&store, &existing, dir.path().join("missing-root"));
    let cwd = task.worktree.as_ref().unwrap().join("src/nested");
    let admission = crate::store::admission::lock_checkout(&store, &cwd).unwrap();
    let mut earlier = unpublished_conversation(None, None, 1);
    earlier.cwd = cwd;
    earlier.work_source = None;
    store.seed_unplaced_task(&task);
    let register = || {
        crate::store::ownership::place_task(
            &store,
            &task.id,
            task.worktree.as_ref().unwrap(),
            &task.workspace_slug,
            &pr,
        )
    };
    assert!(register().is_err());
    assert!(store.task(&task.id).unwrap().unwrap().worktree.is_none());
    assert!(store.task_prs(&task.id).unwrap().is_empty());
    drop(admission);
    let earlier = crate::store::sessions::create_session(&store, earlier, None).unwrap();
    register().unwrap();
    assert_eq!(
        store.session_task_ids(&earlier.id).unwrap(),
        vec![task.id.clone()]
    );
    assert_eq!(store.session(&earlier.id).unwrap().unwrap(), earlier);
    // Allocating the checkout claims the Task; the earlier session does not.
    assert!(store.task_owned(&task.id).unwrap());
}

#[test]
fn missing_root_exclusion_preserves_taskless_session_admission() {
    let (dir, store, existing) = store_with_task();
    let (task, pr) = unregistered_task(&store, &existing, dir.path().join("missing-root"));
    let mut conversation = unpublished_conversation(None, None, 1);
    conversation.cwd = task.worktree.as_ref().unwrap().join("src/nested");
    conversation.work_source = None;
    let exclusion =
        crate::store::admission::lock_checkout(&store, task.worktree.as_ref().unwrap()).unwrap();
    assert!(crate::store::sessions::create_session(&store, conversation.clone(), None).is_err());
    assert!(store.session(&conversation.id).unwrap().is_none());
    drop(exclusion);
    let session = crate::store::sessions::create_session(&store, conversation, None).unwrap();
    store.seed_unplaced_task(&task);
    crate::store::ownership::place_task(
        &store,
        &task.id,
        task.worktree.as_ref().unwrap(),
        &task.workspace_slug,
        &pr,
    )
    .unwrap();
    assert_eq!(
        store.session_task_ids(&session.id).unwrap(),
        vec![task.id.clone()]
    );
    assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
    // Allocating the checkout claims the Task; the earlier session does not.
    assert!(store.task_owned(&task.id).unwrap());
}

#[test]
fn unrelated_checkout_exclusion_does_not_block_task_registration() {
    let (dir, store, existing) = store_with_task();
    let (task, pr) = unregistered_task(&store, &existing, dir.path().join("independent-checkout"));

    let _unrelated = crate::store::admission::lock_checkout(
        &store,
        &dir.path().join("unrelated-checkout/missing/src"),
    )
    .unwrap();
    store.seed_unplaced_task(&task);
    crate::store::ownership::place_task(
        &store,
        &task.id,
        task.worktree.as_ref().unwrap(),
        &task.workspace_slug,
        &pr,
    )
    .unwrap();
    assert_eq!(
        store.task(&task.id).unwrap().unwrap().worktree,
        task.worktree
    );
    assert!(store.task_prs(&task.id).unwrap().is_empty());
}

#[test]
fn checkout_exclusion_preserves_registration_and_retry() {
    let (dir, store, existing) = store_with_task();
    let (task, pr) = unregistered_task(&store, &existing, dir.path().join("missing-checkout"));

    let mut conversation = unpublished_conversation(None, None, 1);
    conversation.cwd = task.worktree.as_ref().unwrap().join("src");
    conversation.work_source = None;
    let session = crate::store::sessions::create_session(&store, conversation, None).unwrap();
    let exclusion =
        crate::store::admission::lock_checkout(&store, task.worktree.as_ref().unwrap()).unwrap();
    store.seed_unplaced_task(&task);
    let register = || {
        crate::store::ownership::place_task(
            &store,
            &task.id,
            task.worktree.as_ref().unwrap(),
            &task.workspace_slug,
            &pr,
        )
    };
    assert!(register().is_err());
    assert!(store.task(&task.id).unwrap().unwrap().worktree.is_none());
    assert!(store.task_prs(&task.id).unwrap().is_empty());
    assert!(store.session_task_ids(&session.id).unwrap().is_empty());
    assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
    drop(exclusion);
    register().unwrap();
    assert_eq!(
        store.session_task_ids(&session.id).unwrap(),
        vec![task.id.clone()]
    );
    assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
    let work = crate::store::task_work::task_work(&store, &task.id).unwrap();
    assert_eq!(work.sessions.len(), 1);
    assert_eq!(work.sessions[0].id, session.id.as_str());
    // Allocating the checkout claims the Task; the earlier session does not.
    assert!(store.task_owned(&task.id).unwrap());
    assert!(store.task_begun(&task.id).unwrap());
}

#[test]
fn checkout_session_membership_survives_project_transfer_without_binding() {
    let (_dir, store, task_id) = store_with_task();
    let task = store.task(&task_id).unwrap().unwrap();
    let mut conversation = unpublished_conversation(None, None, 1);
    conversation.cwd = task.worktree.as_ref().unwrap().join("src");
    conversation.work_source = None;
    let session = crate::store::sessions::create_session(&store, conversation, None).unwrap();
    let before = crate::store::task_work::task_work(&store, &task_id).unwrap();
    assert_eq!(before.sessions.len(), 1);
    assert_eq!(before.sessions[0].id, session.id.as_str());
    assert_eq!(session.task_id, None);
    assert!(store.task_begun(&task_id).unwrap());

    let successor = ProjectId::new();
    store.conn.lock().unwrap().execute(
            "INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'successor',2)",
            rusqlite::params![successor.as_str(), task.wave_id.as_ref().unwrap().as_str()],
        ).unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE tasks SET project_id=?2 WHERE id=?1",
            rusqlite::params![task_id.as_str(), successor.as_str()],
        )
        .unwrap();

    let retained = store.task(&task_id).unwrap().unwrap();
    assert_eq!(retained.project_id, Some(successor));
    assert_eq!(retained.worktree, task.worktree);
    let after = crate::store::task_work::task_work(&store, &task_id).unwrap();
    assert_eq!(after.sessions, before.sessions);
    assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
}

#[test]
fn checkout_exclusion_preserves_unbound_history_until_binding_retries() {
    let (dir, store, task_id) = store_with_task();
    let task = store.task(&task_id).unwrap().unwrap();
    crate::store::ownership::unassign_task(&store, &task_id).unwrap();
    let mut conversation = unpublished_conversation(None, None, 1);
    conversation.cwd = dir.path().join("elsewhere");
    let session = crate::store::sessions::create_session(&store, conversation, None).unwrap();
    let exclusion =
        crate::store::admission::lock_checkout(&store, task.worktree.as_ref().unwrap()).unwrap();
    assert!(
        crate::store::sessions::bind_session(&store, &session.id, session.captured, &task_id)
            .is_err()
    );
    assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
    assert!(!store.task_owned(&task_id).unwrap());
    drop(exclusion);
    let bound =
        crate::store::sessions::bind_session(&store, &session.id, session.captured, &task_id)
            .unwrap();
    assert_eq!(bound.task_id.as_ref(), Some(&task_id));
    // Binding a session claims nothing.
    assert!(!store.task_owned(&task_id).unwrap());
}

#[test]
fn checkout_exclusion_preserves_bound_session_input_until_retry() {
    let (dir, store, task_id) = store_with_task();
    let task = store.task(&task_id).unwrap().unwrap();
    let mut input = unpublished_conversation(Some(task_id.clone()), None, 1);
    input.cwd = dir.path().join("elsewhere");
    let session = crate::store::sessions::create_session(&store, input, None).unwrap();
    let mut next = session.clone();
    next.artifact_key = Some(crate::session_record::new_artifact_key());
    let exclusion =
        crate::store::admission::lock_checkout(&store, task.worktree.as_ref().unwrap()).unwrap();
    let result =
        crate::store::sessions::replace_session_input(&store, session.captured, next.clone(), None);
    assert!(
        result.is_err(),
        "input changed while its Task checkout was excluded"
    );
    assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
    drop(exclusion);
    let replaced =
        crate::store::sessions::replace_session_input(&store, session.captured, next, None)
            .unwrap();
    assert_eq!(replaced.task_id.as_ref(), Some(&task_id));
    assert_eq!(replaced.cwd, session.cwd);
    assert_ne!(replaced.captured, session.captured);
}

#[test]
fn checkout_exclusion_covers_missing_subdirectories_and_explicit_tasks() {
    let (dir, store, task_id) = store_with_task();
    let task = store.task(&task_id).unwrap().unwrap();
    crate::store::ownership::unassign_task(&store, &task_id).unwrap();
    let exclusion =
        crate::store::admission::lock_checkout(&store, task.worktree.as_ref().unwrap()).unwrap();
    let mut conversation = unpublished_conversation(None, None, 1);
    conversation.cwd = task.worktree.as_ref().unwrap().join("missing/subdirectory");
    assert!(crate::store::sessions::create_session(&store, conversation.clone(), None).is_err());
    assert!(store.session(&conversation.id).unwrap().is_none());
    conversation.cwd = dir.path().join("elsewhere");
    conversation.task_id = Some(task_id.clone());
    assert!(crate::store::sessions::create_session(&store, conversation.clone(), None).is_err());
    assert!(!store.task_owned(&task_id).unwrap());
    drop(exclusion);
    let created = crate::store::sessions::create_session(&store, conversation, None).unwrap();
    assert_eq!(created.task_id.as_ref(), Some(&task_id));
    // A session naming its Task claims nothing.
    assert!(!store.task_owned(&task_id).unwrap());
}

#[test]
fn session_binding_retains_a_task_in_completed_project_history() {
    let (dir, store, task_id) = store_with_task();
    let task = store.task(&task_id).unwrap().unwrap();
    crate::store::ownership::unassign_task(&store, &task_id).unwrap();
    let mut conversation = unpublished_conversation(None, None, 1);
    conversation.cwd = dir.path().to_path_buf();
    let session = crate::store::sessions::create_session(&store, conversation, None).unwrap();
    assert!(!store.task_owned(&task_id).unwrap());

    // Rotation can complete a predecessor after observing untouched backlog.
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE projects SET status='completed' WHERE id=?1",
            [task.project_id.as_ref().unwrap().as_str()],
        )
        .unwrap();
    let bound =
        crate::store::sessions::bind_session(&store, &session.id, session.captured, &task_id)
            .unwrap();
    assert_eq!(bound.task_id.as_ref(), Some(&task_id));
    // Binding a session claims nothing.
    assert!(!store.task_owned(&task_id).unwrap());
    let retained = store.task(&task_id).unwrap().unwrap();
    assert_eq!(retained.project_id, task.project_id);
    assert_eq!(retained.worktree, task.worktree);
    assert_eq!(
        crate::store::sessions::bind_session(&store, &session.id, session.captured, &task_id)
            .unwrap(),
        bound
    );
}

#[test]
fn promotion_directory_survives_an_opening_prepared_before_binding() {
    let (dir, store, task_id) = store_with_task();
    let task = store.task(&task_id).unwrap().unwrap();
    let mut conversation = unpublished_conversation(None, None, 1);
    conversation.interactive = true;
    conversation.cwd = dir.path().to_path_buf();
    let session = crate::store::sessions::create_session(&store, conversation, None).unwrap();
    crate::store::ownership::promote_task(
        &store,
        &task_id,
        None,
        None,
        None,
        Some(&session),
        task.worktree.as_deref(),
        None,
    )
    .unwrap();

    let admitted = store
        .admit_session_workspace(&session, &dir.path().join("old-scope"))
        .unwrap();
    assert_eq!(admitted.id, session.id);
    assert_eq!(admitted.cwd, task.worktree.unwrap());
    assert_eq!(admitted.task_id.as_ref(), Some(&task_id));
    assert_eq!(store.session(&session.id).unwrap().unwrap(), admitted);
    assert_eq!(
        store.task_primary(&task_id).unwrap().unwrap().id,
        session.id
    );
}

#[test]
fn promotion_binding_survives_completion_after_placement_preparation() {
    let (_dir, store, task_id) = store_with_task();
    let task = store.task(&task_id).unwrap().unwrap();
    let pr = store.task_prs(&task_id).unwrap().remove(0);
    let placement = (task.worktree.unwrap(), pr);
    let mut conversation = unpublished_conversation(None, None, 1);
    conversation.interactive = true;
    let session = crate::store::sessions::create_session(&store, conversation, None).unwrap();
    // The caller already prepared placement when completion arrived.
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch(&format!(
            "DELETE FROM task_checkouts WHERE task_id='{task_id}';
             UPDATE tasks SET planning_completed=1,planning_state='completed' WHERE id='{task_id}';"
        ))
        .unwrap();
    for _ in 0..2 {
        let bound = crate::store::ownership::promote_task(
            &store,
            &task_id,
            None,
            None,
            Some(&placement),
            Some(&session),
            None,
            None,
        )
        .unwrap();
        assert!(bound.worktree.is_none());
        assert_eq!(
            store.task_state(&task_id).unwrap(),
            crate::durable::TaskState::Done
        );
        assert_eq!(
            store.task_primary(&task_id).unwrap().unwrap().id,
            session.id
        );
        let current = store.session(&session.id).unwrap().unwrap();
        assert_eq!(current.task_id.as_ref(), Some(&task_id));
        assert_eq!(current.cwd, session.cwd);
    }
    assert!(
        crate::store::ownership::place_task(
            &store,
            &task_id,
            &placement.0,
            &placement.1.slug,
            &placement.1
        )
        .is_err(),
        "ordinary checkout allocation still rejects done Tasks"
    );
}

#[test]
fn bind_preserves_prior_work_and_usage_while_assigning_future_work() {
    let (_dir, store, task_id) = store_with_task();
    let task = store.task(&task_id).unwrap().unwrap();
    let other_wave = WaveId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'other','/repo',1)",
            [other_wave.as_str()],
        )
        .unwrap();
    let open = |id: &str, wave: Option<WaveId>| {
        let session = crate::session::LfSession {
            captured: None,
            caller_artifact_key: None,
            task_id: None,
            wave_id: wave,
            graph_lf_process_id: None,
            work_source: None,
            bound_at: None,
            id: id.to_string().into(),
            artifact_key: Some(crate::session_record::new_artifact_key()),
            input_published: true,
            cwd: "/repo".into(),
            skill: None,
            provider: None,
            model: None,
            node: None,
            iterations: None,
            interactive: true,
            repo: None,
            title: id.to_string(),
            title_source: crate::session::TitleSource::Generated,
            request: None,
            completed_at: None,
            created_at: 1,
        };
        crate::store::sessions::create_session(&store, session, None).unwrap()
    };
    let first = open("orphan", None);
    let replacement = crate::store::sessions::replace_session_input(
        &store,
        first.captured,
        crate::session::LfSession {
            caller_artifact_key: None,
            artifact_key: Some(crate::session_record::new_artifact_key()),
            ..first.clone()
        },
        None,
    )
    .unwrap();
    assert!(
        crate::store::sessions::replace_session_input(
            &store,
            replacement.captured,
            crate::session::LfSession {
                cwd: "/changed".into(),
                provider: Some("changed".into()),
                model: Some("changed".into()),
                ..first.clone()
            },
            None
        )
        .is_err(),
        "an earlier input cannot become a fresh replacement"
    );
    assert_eq!(store.session("orphan").unwrap(), Some(replacement.clone()));
    let elsewhere = open("elsewhere", Some(other_wave));
    let old_runs = store.session_inputs("orphan").unwrap();
    store.record_turn_start("orphan", &"thread".into(), "active", 1);
    store.record_turn_usage(
        "orphan",
        &"thread".into(),
        "active",
        2,
        &serde_json::json!({"usage": {"input_tokens": 20}}),
    );
    let earlier = store.session_history("orphan", 0, 0).unwrap();
    assert!(
        crate::store::sessions::bind_session(&store, "orphan", first.captured, &task_id).is_err()
    );
    let bound =
        crate::store::sessions::bind_session(&store, "orphan", replacement.captured, &task_id)
            .unwrap();
    assert_eq!(bound.artifact_key, replacement.artifact_key);
    assert_eq!(bound.task_id, Some(task_id.clone()));
    assert_eq!(bound.wave_id, task.wave_id.clone());
    assert!(bound.bound_at.is_some());
    let binds = store.bound_sessions().unwrap();
    assert_eq!(binds.len(), 1);
    assert_eq!(binds[0].session_id, "orphan");
    assert_eq!(Some(binds[0].at), bound.bound_at);
    assert_eq!(store.session_inputs("orphan").unwrap(), old_runs);
    assert_eq!(store.session_history("orphan", 0, 0).unwrap(), earlier);
    assert_eq!(
        crate::store::sessions::bind_session(&store, "orphan", replacement.captured, &task_id)
            .unwrap(),
        bound
    );
    // A late observation belongs to the old turn, never the new bind.
    store.record_turn_completion(
        "orphan",
        &"thread".into(),
        "active",
        3,
        &serde_json::json!({"status": "completed"}),
    );
    // Attribution freezes when the attached LfProcess sends the request.
    let process = loopflow_model::id::LfProcessId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&process],
        )
        .unwrap();
    let attachment =
        attachment::claim_session_attachment(&store, "orphan", None, &process).unwrap();
    let origin = store.session_turn_origin("orphan", &attachment).unwrap();
    store.record_session_turn_origin(&"thread".into(), "future", &origin);
    let events = store.session_history("orphan", 0, 0).unwrap();
    let turns = store.session_turns("orphan").unwrap();
    let turn = |id: &str| turns.iter().find(|turn| turn.turn == id).unwrap();
    assert_eq!(turn("active").task_id, None);
    assert_eq!(turn("future").task_id.as_deref(), Some(task_id.as_str()));
    let future = crate::store::sessions::replace_session_input(
        &store,
        replacement.captured,
        crate::session::LfSession {
            caller_artifact_key: None,
            artifact_key: Some(crate::session_record::new_artifact_key()),
            ..bound.clone()
        },
        None,
    )
    .unwrap();
    assert_eq!(future.task_id, Some(task_id.clone()));
    assert_eq!(future.work_source, Some(WorkSource::Bound));
    assert!(store
        .session_inputs("orphan")
        .unwrap()
        .contains(first.input_key().unwrap()));
    // A new input is Session state, not history.
    assert_eq!(store.session_history("orphan", 0, 0).unwrap(), events);
    let refused =
        crate::store::sessions::bind_session(&store, "elsewhere", elsewhere.captured, &task_id)
            .unwrap_err();
    assert!(refused.to_string().contains("another Wave"), "{refused}");
    assert_eq!(store.session("elsewhere").unwrap().unwrap().task_id, None);
    let listed = crate::store::session_projection::sessions(
        &store,
        &crate::session::SessionFilter {
            task: Some(task_id.to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].artifact_key, future.artifact_key);
}

#[test]
fn competing_session_assignments_bind_only_the_winning_task() {
    let (dir, store, task_id) = store_with_task();
    let wave = store.task(&task_id).unwrap().unwrap().wave_id;
    let other_task = TaskId::new();
    let session = crate::store::sessions::create_session(
        &store,
        unpublished_conversation(None, None, 1),
        None,
    )
    .unwrap();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO tasks(id,repository_id,project_id,external_issue_id,issue_identifier,created_at)
                SELECT ?1,repository_id,project_id,?1,?1,1 FROM tasks WHERE id=?2",
                rusqlite::params![other_task.as_str(), task_id.as_str()]).unwrap();
    }
    let barrier = Arc::new(Barrier::new(2));
    let writers = [task_id, other_task].map(|target| {
        let path = dir.path().join("loopflow.db");
        let barrier = barrier.clone();
        let wave = wave.clone();
        let session = session.id.clone();
        thread::spawn(move || {
            let conn = rusqlite::Connection::open(path).unwrap();
            conn.busy_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
            barrier.wait();
            let result = conn.execute(
                "UPDATE agent_sessions SET task_id=?2,wave_id=?3 WHERE id=?1",
                rusqlite::params![
                    session.as_str(),
                    target.as_str(),
                    wave.as_ref().unwrap().as_str()
                ],
            );
            (target, result)
        })
    });
    let results = writers.map(|writer| writer.join().unwrap());
    assert_eq!(
        results.iter().filter(|(_, result)| result.is_ok()).count(),
        1
    );
    let conn = store.conn.lock().unwrap();
    for (target, result) in results {
        let has_session: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=?1)",
                [target.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_session, result.is_ok());
        if let Err(error) = result {
            assert!(error.to_string().contains("binding is permanent"));
        }
    }
}
