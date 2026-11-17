use crate::store::Store;
use loopflow_model::durable::TaskId;
use loopflow_storage::query_plans::{growing_scans, statements};
use loopflow_storage::sqlite::tracker_actions::ActionPhase;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::task::{NewTask, Task};
use loopflow_test_support::TestRepo;

fn task(store: &SqliteStore, repo: &TestRepo) -> Task {
    store
        .create_task(&NewTask {
            repository_id: crate::repository::location::ensure_repository(
                store,
                repo.path().to_str().unwrap(),
            )
            .unwrap(),
            id: TaskId::new(),
            project_id: None,
            title: "Saved task".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap()
}

#[test]
fn disconnected_planning_keeps_sent_and_conflicted_actions() {
    let home = tempfile::tempdir().unwrap();
    let store = Store {
        sqlite: crate::store::opening::open_ephemeral(&home.path().join("registry.db")).unwrap(),
    };
    let repo = TestRepo::new();
    let task = task(&store.sqlite, &repo);
    {
        let conn = store.sqlite.conn.lock().unwrap();
        let completed = r#"{"field":"disposition","value":{"planning_state":"completed","planning_completed":true,"planning_completed_at":null}}"#;
        let started = r#"{"revision":"r2","value":{"field":"disposition","value":{"planning_state":"started","planning_completed":false,"planning_completed_at":null}}}"#;
        for (id, phase, conflict) in [
            ("00000000-0000-4000-8000-000000000001", "sent", None),
            (
                "00000000-0000-4000-8000-000000000002",
                "conflicted",
                Some(started),
            ),
            ("00000000-0000-4000-8000-000000000003", "queued", None),
        ] {
            conn.execute("INSERT INTO tracker_actions(id,task_id,kind,value_json,phase,tracker,conflict_json) VALUES(?1,?2,'state',?3,?4,'linear',?5)", rusqlite::params![id, task.id.as_str(), completed, phase, conflict]).unwrap();
        }
    }
    let open = store.sqlite.task_tracker_actions(&task.id).unwrap();
    let connected = open.status(true);
    assert!(connected
        .actions
        .iter()
        .any(|action| action.id == "00000000-0000-4000-8000-000000000003"));
    let disconnected = store.task_tracker_sync(&task.id).unwrap();
    assert_eq!(
        disconnected
            .actions
            .iter()
            .map(|action| (action.id.as_str(), action.phase))
            .collect::<Vec<_>>(),
        [
            ("00000000-0000-4000-8000-000000000001", ActionPhase::Sent),
            (
                "00000000-0000-4000-8000-000000000002",
                ActionPhase::Conflicted
            ),
        ]
    );
    assert_eq!(disconnected.actions[1].change, "complete");
    assert_eq!(
        disconnected.actions[1].tracker_value,
        Some(serde_json::json!("started"))
    );
}

#[test]
fn a_git_title_that_lost_shows_as_conflicted_until_saved_again() {
    let home = tempfile::tempdir().unwrap();
    let store = Store {
        sqlite: crate::store::opening::open_ephemeral(&home.path().join("registry.db")).unwrap(),
    };
    let repo = TestRepo::new();
    let task = task(&store.sqlite, &repo);
    let record = |id: &str, title: &str, clock: i64, parents: &[&str]| {
        let mutation = serde_json::json!({
            "edit": {"target": task.id.as_str(), "change": {"field": "task_title", "value": title}},
            "clock": clock,
            "parents": parents,
        });
        store
            .sqlite
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO planning_peer_changes(id,record_table,mutation) VALUES(?1,'tasks',?2)",
                rusqlite::params![id, mutation.to_string()],
            )
            .unwrap();
    };
    // Creation journaled the first title; two Machines then rename the Task
    // without seeing each other, and the later clock wins.
    let (created, clock): (String, i64) = store
        .sqlite
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT id,clock FROM planning_peer_changes WHERE object_id=?1 AND field='task_title'",
            [task.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(store
        .task_tracker_sync(&task.id)
        .unwrap()
        .actions
        .is_empty());
    record(
        "00000000-0000-4000-8000-00000000000a",
        "Mine",
        clock + 1,
        &[&created],
    );
    record(
        "00000000-0000-4000-8000-00000000000b",
        "Theirs",
        clock + 2,
        &[&created],
    );
    let sync = store.task_tracker_sync(&task.id).unwrap();
    assert_eq!(sync.actions.len(), 1);
    let lost = &sync.actions[0];
    assert_eq!(
        (lost.tracker.as_str(), lost.phase, lost.change.as_str()),
        ("git", ActionPhase::Conflicted, "title")
    );
    assert_eq!(lost.local_value, "Mine");
    assert_eq!(lost.tracker_value, Some(serde_json::json!("Theirs")));
    assert_eq!(
        lost.line(),
        r#"title · conflicted · lost: "Mine" · git: "Theirs""#
    );

    record(
        "00000000-0000-4000-8000-00000000000c",
        "Mine again",
        clock + 3,
        &[
            "00000000-0000-4000-8000-00000000000a",
            "00000000-0000-4000-8000-00000000000b",
        ],
    );
    assert!(store
        .task_tracker_sync(&task.id)
        .unwrap()
        .actions
        .is_empty());
}

/// Two Machines rename the Task concurrently; "Theirs" wins on clock.
fn git_rename_conflict(store: &Store, task: &Task) {
    let conn = store.sqlite.conn.lock().unwrap();
    let (created, clock): (String, i64) = conn
        .query_row(
            "SELECT id,clock FROM planning_peer_changes WHERE object_id=?1 AND field='task_title'",
            [task.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    for (id, title, at) in [
        ("00000000-0000-4000-8000-00000000000a", "Mine", clock + 1),
        ("00000000-0000-4000-8000-00000000000b", "Theirs", clock + 2),
    ] {
        let mutation = serde_json::json!({
            "edit": {"target": task.id.as_str(), "change": {"field": "task_title", "value": title}},
            "clock": at,
            "parents": [created],
        });
        conn.execute(
            "INSERT INTO planning_peer_changes(id,record_table,mutation) VALUES(?1,'tasks',?2)",
            rusqlite::params![id, mutation.to_string()],
        )
        .unwrap();
    }
    conn.execute(
        "UPDATE tasks SET issue_title='Theirs' WHERE id=?1",
        [task.id.as_str()],
    )
    .unwrap();
}

#[test]
fn a_git_value_that_lost_is_answered_by_keeping_theirs_or_using_mine() {
    use loopflow_storage::planning_sync::{LostAnswer, TrackerKind};
    for (answer, title) in [
        (LostAnswer::KeepTheirs, "Theirs"),
        (LostAnswer::UseMine, "Mine"),
    ] {
        let home = tempfile::tempdir().unwrap();
        let store = Store {
            sqlite: crate::store::opening::open_ephemeral(&home.path().join("registry.db"))
                .unwrap(),
        };
        let repo = TestRepo::new();
        let task = task(&store.sqlite, &repo);
        git_rename_conflict(&store, &task);
        let repository = repo.path().to_str().unwrap();
        assert_eq!(
            store
                .sqlite
                .repository_tracker_actions(repository, TrackerKind::Git)
                .unwrap()
                .len(),
            1
        );
        let lost = store
            .sqlite
            .repository_tracker_actions(repository, TrackerKind::Git)
            .unwrap();
        store.sqlite.answer_lost(&lost, answer).unwrap();
        assert!(store
            .task_tracker_sync(&task.id)
            .unwrap()
            .actions
            .is_empty());
        assert!(store
            .sqlite
            .repository_tracker_actions(repository, TrackerKind::Git)
            .unwrap()
            .is_empty());
        assert_eq!(store.sqlite.task(&task.id).unwrap().unwrap().title, title);
    }
}

#[test]
fn planning_receipt_queries_stay_batched_across_repositories() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("registry.db")).unwrap();
    let first = task(&store, &TestRepo::new());
    let second = task(&store, &TestRepo::new());
    let single = statements(&store, || {
        crate::store::ownership::task_detail_facts(&store, &[&first]).unwrap();
    });
    let multiple = statements(&store, || {
        let facts = crate::store::ownership::task_detail_facts(&store, &[&first, &second]).unwrap();
        assert_eq!(facts.tracker_actions.len(), 2);
        assert_ne!(
            facts.tracker_actions[&first.id].repo,
            facts.tracker_actions[&second.id].repo
        );
    });
    assert_eq!(single.len(), multiple.len());
    assert!(growing_scans(&store, &multiple).is_empty());
}

/// A Wave and one Project in `repo`, named "Theirs".
fn project(store: &SqliteStore, repo: &str) -> String {
    let conn = store.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO waves(id,name,repo,created_at) VALUES('wave-answer','answer',?1,1)",
        [repo],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO projects(id,wave_id,project_name,created_at) VALUES('project-answer','wave-answer','Theirs',1)",
        [],
    )
    .unwrap();
    "project-answer".into()
}

#[test]
fn a_project_name_linear_won_is_answered_from_its_line() {
    use loopflow_model::durable::ProjectId;
    use loopflow_storage::planning_sync::{LostAnswer, TrackerKind};
    for (answer, name, queued) in [
        (LostAnswer::KeepTheirs, "Theirs", 0),
        (LostAnswer::UseMine, "Mine", 1),
    ] {
        let home = tempfile::tempdir().unwrap();
        let store = Store {
            sqlite: crate::store::opening::open_ephemeral(&home.path().join("registry.db"))
                .unwrap(),
        };
        let project = ProjectId::from_raw(project(&store.sqlite, "/repo-answer"));
        // Bound to a Linear Project, so a saved name queues for Linear.
        store.sqlite.conn.lock().unwrap().execute_batch(
            "INSERT INTO pm_projects(repo,provider,id,observed_at) VALUES('/repo-answer','linear','linear-answer',1);
             UPDATE projects SET external_project_id='linear-answer' WHERE id='project-answer';",
        )
        .unwrap();
        store.sqlite.conn.lock().unwrap().execute(
            "INSERT INTO tracker_actions(id,project_id,subject,kind,value_json,phase,tracker,conflict_json)
             VALUES('00000000-0000-4000-8000-0000000000d1',?1,'project_name','field',?2,'conflicted','linear',?3)",
            rusqlite::params![
                project.as_str(),
                r#"{"field":"project_name","value":"Mine"}"#,
                r#"{"revision":"r2","value":{"field":"project_name","value":"Theirs"}}"#,
            ],
        )
        .unwrap();
        let lines = store
            .sqlite
            .repository_tracker_actions("/repo-answer", TrackerKind::Linear)
            .unwrap();
        assert_eq!(
            lines.iter().map(|line| line.phase).collect::<Vec<_>>(),
            [ActionPhase::Conflicted]
        );
        store.sqlite.answer_lost(&lines[..1], answer).unwrap();
        let open = store
            .sqlite
            .repository_tracker_actions("/repo-answer", TrackerKind::Linear)
            .unwrap();
        assert!(open.iter().all(|line| line.phase == ActionPhase::Queued));
        assert_eq!(open.len(), queued);
        let saved: String = store
            .sqlite
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT project_name FROM projects WHERE id=?1",
                [project.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(saved, name);
    }
}

#[test]
fn a_git_project_head_that_lost_is_answered_with_a_merge() {
    use loopflow_storage::planning_sync::{LostAnswer, TrackerKind};
    let home = tempfile::tempdir().unwrap();
    let store = Store {
        sqlite: crate::store::opening::open_ephemeral(&home.path().join("registry.db")).unwrap(),
    };
    let project = project(&store.sqlite, "/repo-answer");
    {
        let conn = store.sqlite.conn.lock().unwrap();
        for (id, name, clock, parents) in [
            ("00000000-0000-4000-8000-0000000000e0", "First", 1, vec![]),
            (
                "00000000-0000-4000-8000-0000000000e1",
                "Mine",
                2,
                vec!["00000000-0000-4000-8000-0000000000e0"],
            ),
            (
                "00000000-0000-4000-8000-0000000000e2",
                "Theirs",
                3,
                vec!["00000000-0000-4000-8000-0000000000e0"],
            ),
        ] {
            let mutation = serde_json::json!({
                "edit": {"target": project, "change": {"field": "project_name", "value": name}},
                "clock": clock,
                "parents": parents,
            });
            conn.execute(
                "INSERT INTO planning_peer_changes(id,record_table,mutation) VALUES(?1,'projects',?2)",
                rusqlite::params![id, mutation.to_string()],
            )
            .unwrap();
        }
    }
    let lines = store
        .sqlite
        .repository_tracker_actions("/repo-answer", TrackerKind::Git)
        .unwrap();
    assert_eq!(
        lines
            .iter()
            .map(|line| line.id.as_str())
            .collect::<Vec<_>>(),
        ["00000000-0000-4000-8000-0000000000e1"]
    );
    store
        .sqlite
        .answer_lost(&lines[..1], LostAnswer::KeepTheirs)
        .unwrap();
    assert!(store
        .sqlite
        .repository_tracker_actions("/repo-answer", TrackerKind::Git)
        .unwrap()
        .is_empty());
    let merges: i64 = store
        .sqlite
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM planning_peer_changes WHERE object_id=?1 AND json_array_length(mutation,'$.parents')=2",
            [project.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(merges, 1);
}
