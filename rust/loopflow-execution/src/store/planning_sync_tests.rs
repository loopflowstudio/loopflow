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
            conn.execute("INSERT INTO tracker_actions(id,task_id,kind,value_json,phase,provider,conflict_json) VALUES(?1,?2,'state',?3,?4,'linear',?5)", rusqlite::params![id, task.id.as_str(), completed, phase, conflict]).unwrap();
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
        r#"title · conflicted · lost here: "Mine" · git: "Theirs""#
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
