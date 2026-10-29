use crate::id::WaveId;
use crate::store::sqlite::SqliteStore;
use crate::store::{FrontierAdvance, PlanningState, Store};
use crate::test_linear_input::SeedLinear;
use rusqlite::{params, Connection};
use serde_json::json;
#[tokio::test]
async fn migration_preserves_planning_identity_and_removes_snapshot_storage() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join(".lf/loopflow.db");
    std::fs::create_dir_all(database.parent().unwrap()).unwrap();
    let wave = WaveId::new();
    let snapshot = json!({"projects":[{
        "id":"project", "slug":"chapter", "name":"Chapter", "summary":"Proof",
        "metric_targets":[], "workflow":"feature", "status":"started", "krs":[],
        "initiative_ids":["initiative"], "team_ids":["team"]
    }],"items":[{
        "id":"issue", "identifier":"FIX-1", "url":null, "name":"Retained title",
        "description":"Retained notes", "rank":2, "completed":false, "state":"unstarted",
        "project_id":"project", "project":"chapter", "team_id":"team", "assignee":null
    }]});
    {
        let conn = Connection::open(&database).unwrap();
        crate::store::migrations::apply_released_planning_fixture(&conn);
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'product','/repo',1)",
            [wave.as_str()],
        )
        .unwrap();
        conn.execute("INSERT INTO pm_snapshots(wave_id,provider,initiative,synced_at,payload) VALUES(?1,'linear','initiative',42,?2)",params![wave,snapshot.to_string()]).unwrap();
    }
    // Upgrades of the shared store belong to the install switch.
    let sqlite =
        SqliteStore::open_with(&database, directory.path(), FrontierAdvance::Authorized).unwrap();
    sqlite.apply_migration_for_test("planning_peers").unwrap();
    sqlite
        .apply_migration_for_test("task_checkout_machine")
        .unwrap();
    let store = Store { sqlite };
    let observation = store.linear_issue_evidence("/repo", "fix-1").await.unwrap();
    assert_eq!(observation.state, PlanningState::Available);
    let detail: crate::test_linear_input::IssueFixture =
        loopflow_test_support::retained_task_input(&database, "/repo", "fix-1");
    let list = crate::test_linear_input::InventoryFixture {
        wave_id: wave.clone(),
        provider: "linear".into(),
        initiative: "initiative".into(),
        synced_at: 42,
        snapshot: serde_json::from_value(snapshot).unwrap(),
    };
    assert_eq!(detail.observed_at, 42);
    assert_eq!(detail.item, list.snapshot.items[0]);
    assert_eq!(detail.item.name, "Retained title");
    assert_eq!(detail.item.description, "Retained notes");
    assert_eq!(detail.project.as_ref(), Some(&list.snapshot.projects[0]));
    assert_eq!(store.get_wave(&wave).await.unwrap().unwrap().id(), &wave);
    assert!(store.list_tasks(None).await.unwrap().is_empty());
    assert!(!Connection::open(&database)
        .unwrap()
        .prepare("SELECT payload FROM pm_snapshots")
        .is_ok());

    // A delayed list cannot resurrect a confirmed removal in either reader.
    store.sqlite.conn.lock().unwrap().execute("INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES(?1,'issue','FIX-1',1)", [&wave]).unwrap();
    store.seed_inventory(list, None).await.unwrap();
    let removed = store.linear_issue_evidence("/repo", "FIX-1").await.unwrap();
    assert_eq!(removed.state, PlanningState::Removed);
    assert_eq!(removed.observed_at, Some(detail.observed_at));
    assert_eq!(
        loopflow_test_support::retained_task_input::<crate::test_linear_input::IssueFixture>(
            &database, "/repo", "FIX-1"
        ),
        detail
    );
    assert!(store.list_tasks(None).await.unwrap().is_empty());
}
