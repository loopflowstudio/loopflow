use crate::store::Store;
use loopflow_model::durable::TaskId;
use loopflow_storage::planning_sync::PlanningSyncState;
use loopflow_storage::query_plans::{growing_scans, statements};
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
fn disconnected_planning_keeps_uncertain_and_conflicting_receipts() {
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
            ("00000000-0000-4000-8000-000000000001", "attempted", None),
            (
                "00000000-0000-4000-8000-000000000002",
                "acknowledged",
                Some(started),
            ),
            ("00000000-0000-4000-8000-000000000003", "pending", None),
        ] {
            conn.execute("INSERT INTO tracker_effects(id,task_id,kind,value_json,phase,provider,conflict_json) VALUES(?1,?2,'state',?3,?4,'linear',?5)", rusqlite::params![id, task.id.as_str(), completed, phase, conflict]).unwrap();
        }
    }
    let receipts = store.sqlite.task_planning_receipts(&task.id).unwrap();
    let connected = receipts.clone().status(true);
    assert!(connected
        .changes
        .iter()
        .any(|change| change.id == "00000000-0000-4000-8000-000000000003"));
    let disconnected = store.task_planning_sync(&task.id).unwrap();
    assert!(!disconnected.connected);
    assert_eq!(
        disconnected
            .changes
            .iter()
            .map(|change| (change.id.as_str(), change.state))
            .collect::<Vec<_>>(),
        [
            (
                "00000000-0000-4000-8000-000000000001",
                PlanningSyncState::Uncertain
            ),
            (
                "00000000-0000-4000-8000-000000000002",
                PlanningSyncState::AdoptedLinear
            ),
        ]
    );
    assert_eq!(
        disconnected.changes[1].linear_value,
        Some(serde_json::json!("started"))
    );
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
        assert_eq!(facts.planning_receipts.len(), 2);
        assert_ne!(
            facts.planning_receipts[&first.id].repo,
            facts.planning_receipts[&second.id].repo
        );
    });
    assert_eq!(single.len(), multiple.len());
    assert!(growing_scans(&store, &multiple).is_empty());
}
