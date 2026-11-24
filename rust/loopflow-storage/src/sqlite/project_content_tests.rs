use crate::planning_sync::PlanningChange;
use crate::work::edit::WorkValue::{ProjectDescription, ProjectName, ProjectSummary};
use crate::work::wave::Wave;
use crate::StoreResult;
use loopflow_model::durable::ProjectId;
use loopflow_model::id::WaveId;

use super::SqliteStore;

impl SqliteStore {
    fn pending_project_changes(&self, project: &ProjectId) -> StoreResult<Vec<PlanningChange>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        super::tracker_actions::queued_changes_in(
            &conn,
            &super::tracker_actions::ActionOwner::Project(project.clone()),
        )
    }
}

#[test]
fn project_name_receipt_does_not_reconstruct_unrelated_order_evidence() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
    let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
    crate::test_fixture::wave(&store, &wave);
    let linear = wave.id().to_string();
    let observed = serde_json::from_value(serde_json::json!({
        "id":linear, "revision":"2026-10-10T12:00:00Z", "slug":"product", "name":"Product", "summary":"",
        "workflow":"", "status":"started", "team_ids":["team"]
    })).unwrap();
    crate::sqlite::linear_events::accept_linear_project_in(
        &store,
        wave.id(),
        &observed,
        17,
        crate::planning_sync::TrackerKind::Linear,
    )
    .unwrap();
    let project = store.project_of_wave(wave.id()).unwrap().unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE tracker_projects SET task_order_json='{}' WHERE id=?1",
            [&linear],
        )
        .unwrap();
    store
        .edit_project_fields(&project.id, &[ProjectName(Some("Renamed".into()))])
        .unwrap();
    let change = store
        .pending_project_changes(&project.id)
        .unwrap()
        .remove(0);
    assert_eq!(
        serde_json::to_value(&change.base).unwrap(),
        serde_json::json!({"revision":"2026-10-10T12:00:00Z", "value":{"field":"project_name","value":"Product"}})
    );
    let action = store.tracker_action(&change.id).unwrap().unwrap();
    assert!(store.attempt_tracker_action(&action).unwrap());
    assert_eq!(store.project(&project.id).unwrap().unwrap().name, "Renamed");
    let retained: (String, i64) = store
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT task_order_json,observed_at FROM tracker_projects WHERE id=?1",
            [&linear],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(retained, ("{}".into(), 17));
}

#[test]
fn project_prose_unobserved_baseline_stays_unknown_at_the_same_revision() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
    let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
    crate::test_fixture::wave(&store, &wave);
    let linear = wave.id().to_string();
    let mut observed: crate::planning::TrackerProject = serde_json::from_value(serde_json::json!({
        "id":linear, "revision":"2026-10-10T12:00:00Z", "slug":"product", "name":"Product", "summary":"",
        "workflow":"", "status":"started", "team_ids":["team"]
    })).unwrap();
    crate::sqlite::linear_events::accept_linear_project_in(
        &store,
        wave.id(),
        &observed,
        1,
        crate::planning_sync::TrackerKind::Linear,
    )
    .unwrap();
    let project = store.project_of_wave(wave.id()).unwrap().unwrap();
    store
        .edit_project_fields(
            &project.id,
            &[ProjectDescription(Some("Saved without a baseline".into()))],
        )
        .unwrap();
    let change = store
        .pending_project_changes(&project.id)
        .unwrap()
        .remove(0);
    assert_eq!(change.base, None);
    observed.description = Some(String::new());
    crate::sqlite::linear_events::accept_linear_project_in(
        &store,
        wave.id(),
        &observed,
        2,
        crate::planning_sync::TrackerKind::Linear,
    )
    .unwrap();
    // The unsent save is sent after the observed prose, so it wins and
    // rebases onto the value it will replace.
    assert_eq!(
        store.project(&project.id).unwrap().unwrap().description,
        "Saved without a baseline"
    );
    let conn = store.conn.lock().unwrap();
    let (conflict, base): (Option<String>, String) = conn
        .query_row(
            "SELECT conflict_json,base_json FROM tracker_actions WHERE id=?1",
            [&change.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(conflict.is_none());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&base).unwrap()["value"]["value"],
        ""
    );
}

#[test]
fn project_prose_partial_observation_retains_its_original_baseline_and_age() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
    let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
    crate::test_fixture::wave(&store, &wave);
    let linear = wave.id().to_string();
    let mut observed: crate::planning::TrackerProject = serde_json::from_value(serde_json::json!({
        "id":linear, "revision":"2026-10-10T12:00:00Z", "slug":"product", "name":"Product", "summary":"", "description":"Baseline",
        "workflow":"", "status":"started", "team_ids":["team"]
    })).unwrap();
    crate::sqlite::linear_events::accept_linear_project_in(
        &store,
        wave.id(),
        &observed,
        17,
        crate::planning_sync::TrackerKind::Linear,
    )
    .unwrap();
    let project = store.project_of_wave(wave.id()).unwrap().unwrap();
    observed.description = None;
    observed.revision = Some("2026-10-10T12:01:00Z".into());
    observed.summary = "New summary".into();
    crate::sqlite::linear_events::accept_linear_project_in(
        &store,
        wave.id(),
        &observed,
        29,
        crate::planning_sync::TrackerKind::Linear,
    )
    .unwrap();
    let saved = store.project(&project.id).unwrap().unwrap();
    assert_eq!(saved.description, "Baseline");
    assert_eq!(saved.summary, "New summary");
    store
        .edit_project_fields(&project.id, &[ProjectDescription(Some("Next save".into()))])
        .unwrap();
    let change = store
        .pending_project_changes(&project.id)
        .unwrap()
        .remove(0);
    assert_eq!(
        serde_json::to_value(&change.base).unwrap(),
        serde_json::json!({"value":{"field":"project_description","value":"Baseline"}, "revision":"2026-10-10T12:00:00Z"})
    );
    let prose: String = store
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT json_object('value',description,'revision',description_revision,'observed_at',description_observed_at) FROM tracker_projects WHERE id=?1",
            [&linear],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&prose).unwrap(),
        serde_json::json!({"value":"Baseline", "revision":"2026-10-10T12:00:00Z", "observed_at":17})
    );
    observed.description = Some("Baseline".into());
    crate::sqlite::linear_events::accept_linear_project_in(
        &store,
        wave.id(),
        &observed,
        30,
        crate::planning_sync::TrackerKind::Linear,
    )
    .unwrap();
    assert_eq!(
        store.project(&project.id).unwrap().unwrap().description,
        "Next save"
    );
    assert_eq!(
        store.pending_project_changes(&project.id).unwrap(),
        vec![change]
    );
}

#[test]
fn project_edit_failure_rolls_back_fields_and_receipts() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
    let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
    crate::test_fixture::wave(&store, &wave);
    let project = loopflow_model::durable::ProjectId::new();
    let conn = store.conn.lock().unwrap();
    conn.execute("INSERT OR REPLACE INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,description)
        VALUES(?1,?2,1,1,'product','Original','')",rusqlite::params![project.as_str(),wave.id()]).unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_project_save BEFORE UPDATE ON projects
        BEGIN SELECT RAISE(ABORT,'injected write failure'); END;",
    )
    .unwrap();
    drop(conn);
    let before = store.revisions().unwrap();
    assert!(store
        .edit_project_fields(
            &project,
            &[
                ProjectName(Some("Changed".into())),
                ProjectSummary("New summary".into())
            ]
        )
        .is_err());
    let read = store.project(&project).unwrap().unwrap();
    assert_eq!(read.name, "Original");
    assert_eq!(read.summary, "");
    assert!(store.pending_project_changes(&project).unwrap().is_empty());
    assert_eq!(store.revisions().unwrap(), before);
}

#[test]
fn unchanged_project_content_saves_nothing_with_and_without_linear_mapping() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("store.db");
    let store = crate::test_fixture::open(&database).unwrap();
    for (name, mapping) in [("offline", None), ("connected", Some("linear-project"))] {
        let wave = Wave::new(WaveId::new(), name.into(), "/repo".into());
        crate::test_fixture::wave(&store, &wave);
        let project = loopflow_model::durable::ProjectId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT OR REPLACE INTO projects(id,wave_id,external_project_id,created_at,updated_at,
             project_slug,project_name,description) VALUES(?1,?2,?3,1,1,?4,?4,'')",
                rusqlite::params![project.as_str(), wave.id(), mapping, name],
            )
            .unwrap();
        let before = store.revisions().unwrap();
        let changes = store.pending_project_changes(&project).unwrap();
        store
            .edit_project_fields(
                &project,
                &[ProjectName(Some(name.into())), ProjectSummary("".into())],
            )
            .unwrap();
        assert_eq!(store.revisions().unwrap(), before);
        assert_eq!(store.pending_project_changes(&project).unwrap(), changes);
    }
}

#[test]
fn a_linked_projects_url_is_kept_from_the_tracker_read() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
    let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
    crate::test_fixture::wave(&store, &wave);
    let read = |revision: &str, url: serde_json::Value| {
        let observed = serde_json::from_value(serde_json::json!({
            "id":wave.id().to_string(), "revision":revision, "slug":"product", "name":"Product",
            "summary":"", "status":"started", "team_ids":["team"], "url":url
        }))
        .unwrap();
        crate::sqlite::linear_events::accept_linear_project_in(
            &store,
            wave.id(),
            &observed,
            17,
            crate::planning_sync::TrackerKind::Linear,
        )
        .unwrap();
    };
    let url = "https://linear.app/loopflow/project/product-1a2b3c4d5e6f";
    read("2026-10-10T12:00:00Z", url.into());
    let project = store.project_of_wave(wave.id()).unwrap().unwrap();
    assert_eq!(
        store.project_url(&project.id).unwrap().as_deref(),
        Some(url)
    );
    // A read from a peer on released lf carries no URL; the known one stays.
    read("2026-10-11T12:00:00Z", serde_json::Value::Null);
    assert_eq!(
        store.project_url(&project.id).unwrap().as_deref(),
        Some(url)
    );
}
