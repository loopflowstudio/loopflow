use axum::http::StatusCode;
use serde_json::json;

use super::test_fixture::{now, Fixture};
use super::{read_task_planning_async, PmRefresh, PM_TEST_CONTEXT};
use crate::pm::test_server::{json_response, spawn, QueuedResponse};
use crate::pm::PmSnapshot;
use crate::store::PmSnapshotRow;

fn issue(project: serde_json::Value) -> serde_json::Value {
    json!({"data":{"issue":{
        "id":"issue-1", "identifier":"FIX-1", "url":null,
        "title":"Inspect a planning-only Task", "description":"Keep this work visible",
        "prioritySortOrder":0.0, "sortOrder":0.0, "assignee":null,
        "state":{"type":"unstarted"}, "team":{"id":"team-1"}, "project":project
    }}})
}

fn team_response() -> QueuedResponse {
    json_response(
        StatusCode::OK,
        json!({"data":{"teams":{"nodes":[{
            "id":"team-1", "name":"Fixture", "key":"FIX",
            "description":"<!-- loopflow-repository: loopflowstudio/fixture -->"
        }]}}}),
    )
}

fn project() -> serde_json::Value {
    json!({"id":"project-1", "name":"Chapter", "description":"", "content":"",
        "initiatives":{"nodes":[{"id":"initiative-1"}]},
        "teams":{"nodes":[{"id":"team-1"}]}})
}

#[tokio::test]
async fn fresh_lookup_and_wave_list_share_planning_without_execution() {
    let fixture = Fixture::new().await;
    let (repo, wave) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let record = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            assert_eq!(record.item.name, "Inspect a planning-only Task");
            let resolved =
                crate::ops::task_pm::resolve_task_async(&repo, "issue-1", PmRefresh::Never)
                    .await
                    .unwrap();
            assert_eq!(resolved.wave, "product");
            assert_eq!(resolved.item, record.item);
            assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
            assert!(fixture.store.list_projects(None).await.unwrap().is_empty());
            assert!(fixture
                .store
                .pm_snapshot(wave.id())
                .await
                .unwrap()
                .is_none());

            // A list carries membership, not another copy of the Task's title.
            let mut snapshot = PmSnapshotRow {
                wave_id: wave.id().clone(),
                provider: "linear".into(),
                initiative: "initiative-1".into(),
                synced_at: record.observed_at,
                snapshot: PmSnapshot {
                    projects: vec![record.project.clone().unwrap()],
                    items: vec![record.item.clone()],
                },
            };
            fixture
                .store
                .put_pm_snapshot(snapshot.clone())
                .await
                .unwrap();
            let mut edited = record.clone();
            edited.item.name = "Changed through detail sync".into();
            edited.observed_at += 1;
            fixture
                .store
                .put_pm_task(&repo.to_string_lossy(), "linear", edited.clone())
                .await
                .unwrap();
            assert_eq!(
                fixture
                    .store
                    .pm_snapshot(wave.id())
                    .await
                    .unwrap()
                    .unwrap()
                    .snapshot
                    .items[0],
                edited.item
            );
            snapshot.snapshot.items.clear();
            snapshot.synced_at += 2;
            fixture.store.put_pm_snapshot(snapshot).await.unwrap();
            assert_eq!(
                read_task_planning_async(&repo, "fix-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                edited
            );
            assert_eq!(
                fixture
                    .store
                    .pm_snapshot(wave.id())
                    .await
                    .unwrap()
                    .unwrap()
                    .snapshot
                    .items
                    .len(),
                1
            );
        })
        .await;
}

#[tokio::test]
async fn projectless_task_is_inspectable_but_cannot_resolve_managed_ownership() {
    let fixture = Fixture::new().await;
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(json!(null))),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let record = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            assert!(record.project.is_none());
            assert!(record.item.project_id.is_none());
            assert!(record.item.project.is_none());
            let error = crate::ops::task_pm::resolve_task_async(&repo, "FIX-1", PmRefresh::Never)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("has no Project"), "{error}");
            assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
        })
        .await;
}

#[tokio::test]
async fn missing_and_unavailable_tasks_do_not_create_planning_or_execution() {
    for (response, diagnostic) in [
        (
            json!({"data":{"issue":null}}),
            "absent from repository planning",
        ),
        (
            json!({"errors":[{"message":"permission denied"}]}),
            "unable to resolve",
        ),
        (
            json!({"data":{"issue":issue(project())["data"]["issue"]},"errors":[{"message":"partial response"}]}),
            "unable to resolve",
        ),
    ] {
        let fixture = Fixture::new().await;
        let (repo, _) = fixture.planning_repo().await;
        fixture.seed(now() + 3600).await;
        let (url, _) = spawn(vec![
            team_response(),
            json_response(StatusCode::OK, response),
        ])
        .await;
        PM_TEST_CONTEXT
            .scope(fixture.context(&url), async {
                let error = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                    .await
                    .unwrap_err();
                assert!(error.to_string().contains(diagnostic), "{error}");
                assert!(fixture
                    .store
                    .pm_task(&repo.to_string_lossy(), "linear", "FIX-1")
                    .await
                    .unwrap()
                    .is_none());
                assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
            })
            .await;
    }
}

#[tokio::test]
async fn failed_refresh_preserves_the_last_observation_and_its_age() {
    let fixture = Fixture::new().await;
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        team_response(),
        json_response(
            StatusCode::OK,
            json!({"errors":[{"message":"provider unavailable"}]}),
        ),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let original = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            assert!(read_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .is_err());
            assert_eq!(
                read_task_planning_async(&repo, "issue-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                original
            );
        })
        .await;
}

#[tokio::test]
async fn omitted_detail_fields_do_not_clear_known_planning() {
    let fixture = Fixture::new().await;
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let mut incomplete = issue(project());
    incomplete["data"]["issue"]
        .as_object_mut()
        .unwrap()
        .remove("project");
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        team_response(),
        json_response(StatusCode::OK, incomplete),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let original = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            let error = read_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("missing field `project`"),
                "{error}"
            );
            assert_eq!(
                read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                original
            );
        })
        .await;
}

#[tokio::test]
async fn missing_detail_invalidates_cached_admission_without_claiming_deletion() {
    let fixture = Fixture::new().await;
    let (repo, wave) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        team_response(),
        json_response(StatusCode::OK, json!({"data":{"issue":null}})),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let original = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            assert!(read_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .is_err());
            fixture
                .store
                .put_pm_snapshot(PmSnapshotRow {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative-1".into(),
                    synced_at: original.observed_at,
                    snapshot: PmSnapshot {
                        projects: vec![original.project.unwrap()],
                        items: vec![original.item],
                    },
                })
                .await
                .unwrap();
            assert!(read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            assert!(fixture
                .store
                .pm_snapshot(wave.id())
                .await
                .unwrap()
                .unwrap()
                .snapshot
                .items
                .is_empty());
            assert!(fixture
                .store
                .deleted_task_issues(wave.id())
                .await
                .unwrap()
                .is_empty());
        })
        .await;
}
