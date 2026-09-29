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
        "prioritySortOrder":0.0, "sortOrder":0.0, "updatedAt":"2026-09-29T12:00:00.123Z", "assignee":null,
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
                .unwrap()
                .record;
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
            edited.item.revision = Some("2026-09-29T12:00:00.124Z".into());
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
                    .unwrap()
                    .record,
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
                .unwrap()
                .record;
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
                .unwrap()
                .record;
            assert!(read_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .is_err());
            assert_eq!(
                read_task_planning_async(&repo, "issue-1", PmRefresh::Never)
                    .await
                    .unwrap()
                    .record,
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
    let missing_fields = ["project", "description"];
    let mut responses = vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
    ];
    for field in missing_fields {
        let mut incomplete = issue(project());
        incomplete["data"]["issue"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        responses.extend([team_response(), json_response(StatusCode::OK, incomplete)]);
    }
    let (url, _) = spawn(responses).await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let original = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap()
                .record;
            for field in missing_fields {
                let error = read_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                    .await
                    .unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains(&format!("missing field `{field}`")),
                    "{error}"
                );
                assert_eq!(
                    read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                        .await
                        .unwrap()
                        .record,
                    original
                );
            }
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
                .unwrap()
                .record;
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

#[tokio::test]
async fn automatic_refresh_reports_failure_with_retained_observation_age() {
    let fixture = Fixture::new().await;
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let record: crate::store::PmTaskRecord = serde_json::from_value(json!({
        "item": {"id":"issue-1", "identifier":"FIX-1", "revision":"2026-09-29T12:00:00.123Z",
            "url":null, "name":"Last known title", "description":"", "rank":0,
            "completed":false,"state":"unstarted","project_id":null,"project":null,
            "team_id":"team-1","assignee":null},
        "project":null, "observed_at":now() - super::PM_SOFT_STALE_SECS - 1
    }))
    .unwrap();
    fixture
        .store
        .put_pm_task(&repo.to_string_lossy(), "linear", record.clone())
        .await
        .unwrap();
    let (url, _) = spawn(vec![
        team_response(),
        json_response(
            StatusCode::OK,
            json!({"errors":[{"message":"provider unavailable"}]}),
        ),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let read = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            assert!(read.is_stale());
            assert!(read.refresh_error.unwrap().contains("provider unavailable"));
            assert_eq!(read.record, record);
            assert_eq!(
                fixture
                    .store
                    .pm_task(&repo.to_string_lossy(), "linear", "FIX-1")
                    .await
                    .unwrap(),
                Some(record)
            );
        })
        .await;
}

#[tokio::test]
async fn provider_revisions_and_webhooks_converge_without_execution() {
    use crate::webhook::{ingest_event, parse_event};
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
            let original = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap()
                .record;
            let scope = repo.to_string_lossy();
            let mut list = PmSnapshotRow {
                wave_id: wave.id().clone(),
                provider: "linear".into(),
                initiative: "initiative-1".into(),
                synced_at: original.observed_at,
                snapshot: PmSnapshot {
                    projects: vec![original.project.clone().unwrap()],
                    items: vec![original.item.clone()],
                },
            };
            fixture.store.put_pm_snapshot(list.clone()).await.unwrap();
            let mut confirmed = original.clone();
            confirmed.item.revision = Some("2026-09-29T12:00:00.124Z".into());
            confirmed.item.name = "Confirmed mutation".into();
            // A newer provider revision wins even when its request started earlier.
            confirmed.observed_at -= 1;
            fixture
                .store
                .put_pm_task(&scope, "linear", confirmed.clone())
                .await
                .unwrap();
            list.synced_at += 10;
            fixture.store.put_pm_snapshot(list.clone()).await.unwrap();
            assert_eq!(
                fixture
                    .store
                    .pm_task(&scope, "linear", "FIX-1")
                    .await
                    .unwrap(),
                Some(confirmed.clone())
            );
            assert_eq!(
                fixture
                    .store
                    .pm_snapshot(wave.id())
                    .await
                    .unwrap()
                    .unwrap()
                    .snapshot
                    .items[0],
                confirmed.item
            );
            let mut conflicting = confirmed.clone();
            conflicting.item.name = "Contradiction at the same revision".into();
            assert!(fixture
                .store
                .put_pm_task(&scope, "linear", conflicting)
                .await
                .is_err());
            assert_eq!(
                fixture
                    .store
                    .pm_task(&scope, "linear", "FIX-1")
                    .await
                    .unwrap(),
                Some(confirmed.clone())
            );
            let change = json!({"type":"Issue","action":"update","data":{"id":"issue-1",
            "updatedAt":"2026-09-29T12:00:00.125Z"},"updatedFrom":{"stateId":"old"}});
            let (event, _) = parse_event(change.to_string().as_bytes()).unwrap();
            ingest_event(
                &fixture.store,
                event,
                "viewer",
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();
            let mut delayed = confirmed.clone();
            delayed.observed_at += 20;
            fixture
                .store
                .put_pm_task(&scope, "linear", delayed)
                .await
                .unwrap();
            assert!(fixture
                .store
                .pm_task(&scope, "linear", "FIX-1")
                .await
                .unwrap()
                .is_none());
            assert!(fixture
                .store
                .pm_snapshot(wave.id())
                .await
                .unwrap()
                .unwrap()
                .snapshot
                .items
                .is_empty());
            confirmed.item.revision = Some("2026-09-29T12:00:00.125Z".into());
            confirmed.item.state = Some("completed".into());
            confirmed.item.completed = true;
            fixture
                .store
                .put_pm_task(&scope, "linear", confirmed.clone())
                .await
                .unwrap();
            // Duplicate and older webhook delivery cannot invalidate an equal/newer observation.
            for revision in ["2026-09-29T12:00:00.125Z", "2026-09-29T12:00:00.123Z"] {
                let mut replay = change.clone();
                replay["data"]["updatedAt"] = json!(revision);
                let (event, _) = parse_event(replay.to_string().as_bytes()).unwrap();
                ingest_event(
                    &fixture.store,
                    event,
                    "viewer",
                    time::OffsetDateTime::now_utc(),
                )
                .await
                .unwrap();
            }
            assert_eq!(
                fixture
                    .store
                    .pm_task(&scope, "linear", "issue-1")
                    .await
                    .unwrap(),
                Some(confirmed.clone())
            );
            let mut removal = change;
            removal["action"] = json!("remove");
            let (event, _) = parse_event(removal.to_string().as_bytes()).unwrap();
            ingest_event(
                &fixture.store,
                event,
                "viewer",
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();
            confirmed.item.revision = Some("2026-09-29T12:00:00.126Z".into());
            fixture
                .store
                .put_pm_task(&scope, "linear", confirmed)
                .await
                .unwrap();
            fixture.store.put_pm_snapshot(list).await.unwrap();
            assert!(fixture
                .store
                .pm_task(&scope, "linear", "FIX-1")
                .await
                .unwrap()
                .is_none());
            assert!(fixture
                .store
                .pm_snapshot(wave.id())
                .await
                .unwrap()
                .unwrap()
                .snapshot
                .items
                .is_empty());
            // A removal received before acquisition also fences future list/detail writes.
            removal["data"]["id"] = json!("issue-2");
            let (event, _) = parse_event(removal.to_string().as_bytes()).unwrap();
            ingest_event(
                &fixture.store,
                event,
                "viewer",
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();
            let mut uncached = original;
            uncached.item.id = "issue-2".into();
            uncached.item.identifier = "FIX-2".into();
            fixture
                .store
                .put_pm_task(&scope, "linear", uncached)
                .await
                .unwrap();
            assert!(fixture
                .store
                .pm_task(&scope, "linear", "FIX-2")
                .await
                .unwrap()
                .is_none());
            assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
        })
        .await;
}
