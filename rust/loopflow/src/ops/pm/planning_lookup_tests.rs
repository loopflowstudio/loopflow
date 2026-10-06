use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::json;
use tokio::sync::Barrier;

use super::test_fixture::{now, Fixture};
use super::{inspect_task_planning_async, read_task_planning_async, PmRefresh, PM_TEST_CONTEXT};
use crate::id::WaveId;
use crate::pm::test_server::{json_response, spawn, QueuedResponse};
use crate::pm::PmSnapshot;
use crate::store::{PmSnapshotRow, PmTaskRecord};

fn issue(project: serde_json::Value) -> serde_json::Value {
    json!({"data":{"issue":{
        "id":"issue-1", "identifier":"FIX-1", "url":null, "branchName":"dev/fix-1-existing",
        "title":"Inspect a planning-only Task", "description":"Keep this work visible",
        "completedAt": null, "prioritySortOrder":0.0, "sortOrder":0.0, "updatedAt":"2026-09-29T12:00:00.123Z", "assignee":null,
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
    json!({"id":"project-1", "name":"Chapter", "status":{"type":"started"}, "updatedAt":"2026-09-29T11:00:00Z", "description":"", "content":"",
        "initiatives":{"nodes":[{"id":"initiative-1"}]},
        "teams":{"nodes":[{"id":"team-1"}]}})
}

fn snapshot(wave: &WaveId, record: &PmTaskRecord) -> PmSnapshotRow {
    PmSnapshotRow {
        wave_id: wave.clone(),
        provider: "linear".into(),
        initiative: "initiative-1".into(),
        synced_at: record.observed_at,
        snapshot: PmSnapshot {
            projects: record.project.clone().into_iter().collect(),
            items: vec![record.item.clone()],
        },
    }
}

#[tokio::test]
async fn fresh_lookup_and_wave_list_share_planning_without_execution() {
    let fixture = Fixture::new().await;
    let (repo, wave) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let mut provider_project = project();
    provider_project["name"] = "Summer work — customer requests".into();
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(provider_project.clone())),
        json_response(StatusCode::OK, issue(provider_project.clone())),
        team_response(),
        json_response(StatusCode::OK, issue(provider_project.clone())),
        json_response(StatusCode::OK, issue(provider_project)),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let record = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            assert_eq!(record.item.name, "Inspect a planning-only Task");
            assert_eq!(
                record.project.as_ref().unwrap().name,
                "Summer work — customer requests"
            );
            assert_eq!(
                record.item.project.as_deref(),
                Some("summer-work-customer-requests")
            );
            assert_eq!(
                record.item.branch_name.as_deref(),
                Some("dev/fix-1-existing")
            );
            let resolved =
                crate::ops::task_pm::resolve_task_async(&repo, "issue-1", PmRefresh::Never)
                    .await
                    .unwrap();
            assert_eq!(resolved.wave, "product");
            assert_eq!(resolved.item, record.item);
            assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
            assert_eq!(fixture.store.list_projects(None).await.unwrap().len(), 1);
            assert!(fixture
                .store
                .pm_snapshot(wave.id())
                .await
                .unwrap()
                .is_none());

            // A list carries membership, not another copy of the Task's title.
            let mut snapshot = snapshot(wave.id(), &record);
            fixture
                .store
                .put_pm_snapshot(snapshot.clone(), None)
                .await
                .unwrap();
            let refreshed = read_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .unwrap();
            assert_eq!(refreshed.project, record.project);
            assert_eq!(refreshed.item, record.item);
            let mut edited = record.clone();
            edited.item.name = "Changed through detail sync".into();
            edited.item.revision = Some("2026-09-29T12:00:00.124Z".into());
            edited.observed_at += 1;
            fixture
                .store
                .put_pm_task(
                    &repo.to_string_lossy(),
                    "linear",
                    edited.clone(),
                    None,
                    None,
                )
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
            fixture.store.put_pm_snapshot(snapshot, None).await.unwrap();
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
async fn rejected_project_snapshot_preserves_durable_project_facts() {
    let fixture = Fixture::new().await;
    let (_repo, wave) = fixture.planning_repo().await;
    let mut snapshot: PmSnapshot = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/task_history_planning.json"
    ))
    .unwrap();
    snapshot.items.clear();
    snapshot.projects.truncate(1);
    snapshot.projects[0].revision = Some("2026-10-05T12:00:00Z".into());
    snapshot.projects[0].initiative_ids = vec!["initiative-1".into()];
    let mut snapshot = PmSnapshotRow {
        wave_id: wave.id().clone(),
        provider: "linear".into(),
        initiative: "initiative-1".into(),
        synced_at: 1,
        snapshot,
    };
    fixture
        .store
        .put_pm_snapshot(snapshot.clone(), None)
        .await
        .unwrap();
    let retained = fixture.store.list_projects(Some(wave.id())).await.unwrap();
    snapshot.snapshot.projects[0].name = "Conflicting name".into();
    snapshot.synced_at = 2;
    assert!(fixture.store.put_pm_snapshot(snapshot, None).await.is_err());
    assert_eq!(
        fixture.store.list_projects(Some(wave.id())).await.unwrap(),
        retained
    );
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
                    .pm_task_observation(&repo.to_string_lossy(), "linear", "FIX-1")
                    .await
                    .unwrap()
                    .record
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
        json_response(StatusCode::OK, issue(project())),
        team_response(),
        json_response(
            StatusCode::OK,
            json!({"errors":[{"message":"provider unavailable"}]}),
        ),
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
            let inspection = inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .unwrap();
            assert_eq!(
                inspection.observation.state,
                crate::store::PlanningState::Unavailable
            );
            assert_eq!(inspection.observation.record.as_ref(), Some(&original));
            assert!(inspection
                .refresh_error
                .unwrap()
                .contains("provider unavailable"));
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
    let missing_fields = [
        "issue",
        "project",
        "description",
        "project.initiatives",
        "project.teams",
        "project.content",
        "project.description",
    ];
    let mut responses = vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        json_response(StatusCode::OK, issue(project())),
    ];
    for field in missing_fields {
        let mut incomplete = issue(project());
        if field == "issue" {
            incomplete["data"].as_object_mut().unwrap().remove(field);
        } else if let Some(field) = field.strip_prefix("project.") {
            incomplete["data"]["issue"]["project"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        } else {
            incomplete["data"]["issue"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        }
        responses.extend([team_response(), json_response(StatusCode::OK, incomplete)]);
    }
    let (url, _) = spawn(responses).await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let original = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            for field in missing_fields {
                let field = field.strip_prefix("project.").unwrap_or(field);
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
                        .unwrap(),
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
        json_response(StatusCode::OK, issue(project())),
        team_response(),
        json_response(StatusCode::OK, json!({"data":{"issue":null}})),
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
                .put_pm_snapshot(snapshot(wave.id(), &original), None)
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
        .put_pm_task(
            &repo.to_string_lossy(),
            "linear",
            record.clone(),
            None,
            None,
        )
        .await
        .unwrap();
    let (url, _) = spawn(vec![
        team_response(),
        json_response(
            StatusCode::OK,
            json!({"errors":[{"message":"provider unavailable"}]}),
        ),
        team_response(),
        json_response(
            StatusCode::OK,
            json!({"errors":[{"message":"provider unavailable"}]}),
        ),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let read = inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            assert!(read.is_stale());
            assert!(read.refresh_error.unwrap().contains("provider unavailable"));
            assert_eq!(read.observation.record.as_ref(), Some(&record));
            assert!(read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap_err()
                .to_string()
                .contains("provider unavailable"));
            assert_eq!(
                read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                record
            );
        })
        .await;
}

#[tokio::test]
async fn provider_revisions_and_change_receipts_converge_without_execution() {
    let fixture = Fixture::new().await;
    let (repo, wave) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        json_response(StatusCode::OK, issue(project())),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let original = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            let scope = repo.to_string_lossy();
            let mut list = snapshot(wave.id(), &original);
            fixture
                .store
                .put_pm_snapshot(list.clone(), None)
                .await
                .unwrap();
            let mut confirmed = original.clone();
            confirmed.item.revision = Some("2026-09-29T12:00:00.124Z".into());
            confirmed.item.name = "Confirmed mutation".into();
            // A newer provider revision wins even when its request started earlier.
            confirmed.observed_at -= 1;
            fixture
                .store
                .put_pm_task(&scope, "linear", confirmed.clone(), None, None)
                .await
                .unwrap();
            list.synced_at += 10;
            fixture
                .store
                .put_pm_snapshot(list.clone(), None)
                .await
                .unwrap();
            assert_eq!(
                read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                confirmed
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
                .put_pm_task(&scope, "linear", conflicting, None, None)
                .await
                .is_err());
            assert_eq!(
                read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                confirmed
            );
            fixture
                .store
                .observe_pm_issue_change("issue-1", Some("2026-09-29T12:00:00.125Z"), false)
                .await
                .unwrap();
            let mut delayed = confirmed.clone();
            delayed.observed_at += 20;
            fixture
                .store
                .put_pm_task(&scope, "linear", delayed, None, None)
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
            confirmed.item.revision = Some("2026-09-29T12:00:00.125Z".into());
            confirmed.item.state = Some("completed".into());
            confirmed.item.completed = true;
            fixture
                .store
                .put_pm_task(&scope, "linear", confirmed.clone(), None, None)
                .await
                .unwrap();
            // Duplicate and older change receipts cannot invalidate an equal/newer observation.
            for revision in ["2026-09-29T12:00:00.125Z", "2026-09-29T12:00:00.123Z"] {
                fixture
                    .store
                    .observe_pm_issue_change("issue-1", Some(revision), false)
                    .await
                    .unwrap();
            }
            assert_eq!(
                read_task_planning_async(&repo, "issue-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                confirmed
            );
            // A content edit without ordering evidence still invalidates planning.
            fixture
                .store
                .observe_pm_issue_change("issue-1", None, false)
                .await
                .unwrap();
            assert!(read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            fixture
                .store
                .put_pm_task(&scope, "linear", confirmed.clone(), None, None)
                .await
                .unwrap();
            assert_eq!(
                read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                confirmed
            );
            fixture
                .store
                .observe_pm_issue_change("issue-1", Some("2026-09-29T12:00:00.125Z"), true)
                .await
                .unwrap();
            confirmed.item.revision = Some("2026-09-29T12:00:00.126Z".into());
            fixture
                .store
                .put_pm_task(&scope, "linear", confirmed, None, None)
                .await
                .unwrap();
            fixture.store.put_pm_snapshot(list, None).await.unwrap();
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
            // A removal received before acquisition also fences future list/detail writes.
            fixture
                .store
                .observe_pm_issue_change("issue-2", Some("2026-09-29T12:00:00.125Z"), true)
                .await
                .unwrap();
            let mut uncached = original;
            uncached.item.id = "issue-2".into();
            uncached.item.identifier = "FIX-2".into();
            fixture
                .store
                .put_pm_task(&scope, "linear", uncached, None, None)
                .await
                .unwrap();
            assert!(read_task_planning_async(&repo, "FIX-2", PmRefresh::Never)
                .await
                .is_err());
            assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
        })
        .await;
}

#[tokio::test]
async fn inspection_retains_invalid_removed_and_absent_facts_without_admitting_work() {
    let fixture = Fixture::new().await;
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        json_response(StatusCode::OK, issue(project())),
        team_response(),
        json_response(StatusCode::OK, json!({"data":{"issue":null}})),
        json_response(StatusCode::OK, json!({"data":{"issue":null}})),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let original = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            fixture
                .store
                .invalidate_pm_task(&repo.to_string_lossy(), "linear", original.clone(), None)
                .await
                .unwrap();
            let invalid = inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                .await
                .unwrap();
            assert_eq!(
                invalid.observation.state,
                crate::store::PlanningState::Invalid
            );
            assert_eq!(invalid.observation.record.as_ref(), Some(&original));
            assert!(read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            let absent = inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .unwrap();
            assert_eq!(
                absent.observation.state,
                crate::store::PlanningState::Absent
            );
            assert_eq!(absent.observation.record.as_ref(), Some(&original));
            // Null is an absence observation, not a confirmed deletion receipt.
            fixture
                .store
                .observe_pm_issue_change("issue-1", None, true)
                .await
                .unwrap();
            let removed = inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .unwrap();
            assert_eq!(
                removed.observation.state,
                crate::store::PlanningState::Removed
            );
            assert_eq!(removed.observation.record.as_ref(), Some(&original));
            assert!(read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
        })
        .await;
}

#[tokio::test]
async fn removal_during_absent_lookup_preserves_confirmed_evidence() {
    let fixture = Fixture::new().await;
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let mut absent = json_response(StatusCode::OK, json!({"data":{"issue":null}}));
    absent.gate = Some((entered.clone(), release.clone()));
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        json_response(StatusCode::OK, issue(project())),
        team_response(),
        absent,
        json_response(StatusCode::OK, json!({"data":{"issue":null}})),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let original = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            let (inspection, ()) = tokio::join!(
                inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Force),
                async {
                    entered.wait().await;
                    fixture
                        .store
                        .observe_pm_issue_change("issue-1", None, true)
                        .await
                        .unwrap();
                    release.wait().await;
                }
            );
            let inspection = inspection.unwrap();
            assert_eq!(
                inspection.observation.state,
                crate::store::PlanningState::Removed
            );
            assert_eq!(inspection.observation.record.as_ref(), Some(&original));
            assert!(inspection.refresh_error.is_none());
            assert!(read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
        })
        .await;
}

#[tokio::test]
async fn project_revisions_order_shared_facts_and_unordered_membership_stays_unresolved() {
    let fixture = Fixture::new().await;
    let (repo, wave) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        json_response(StatusCode::OK, issue(project())),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let original = read_task_planning_async(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            let mut list = snapshot(wave.id(), &original);
            fixture
                .store
                .put_pm_snapshot(list.clone(), None)
                .await
                .unwrap();
            let mut newer = original.clone();
            let project = newer.project.as_mut().unwrap();
            project.revision = Some("2026-09-29T11:00:00.000000001Z".into());
            project.summary = "New Project facts, unchanged issue revision".into();
            project.name = "Renamed chapter".into();
            project.slug = "renamed-chapter".into();
            newer.observed_at -= 1;
            fixture
                .store
                .put_pm_task(&repo.to_string_lossy(), "linear", newer.clone(), None, None)
                .await
                .unwrap();
            // The list arrived later but its provider revision is older.
            list.synced_at += 60;
            fixture
                .store
                .put_pm_snapshot(list.clone(), None)
                .await
                .unwrap();
            let stored = read_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                .await
                .unwrap();
            assert_eq!(stored.project, newer.project);
            assert_eq!(stored.item.project.as_deref(), Some("renamed-chapter"));
            assert_eq!(stored.observed_at, list.synced_at);
            let snapshot = fixture.store.pm_snapshot(wave.id()).await.unwrap().unwrap();
            assert_eq!(
                snapshot.snapshot.projects[0],
                newer.project.clone().unwrap()
            );
            assert_eq!(snapshot.snapshot.items.len(), 1);
            assert_eq!(snapshot.snapshot.items[0], stored.item);
            let mut omitted = list.clone();
            omitted.snapshot.projects.clear();
            omitted.snapshot.items.clear();
            assert!(fixture
                .store
                .put_pm_snapshot(omitted, None)
                .await
                .unwrap_err()
                .to_string()
                .contains("omitted"));
            assert_eq!(
                fixture.store.pm_snapshot(wave.id()).await.unwrap().unwrap(),
                snapshot
            );
            for revision in [
                Some("2026-09-29T11:00:00Z"),
                None,
                Some("2026-09-29T11:00:00.000000001Z"),
                Some("2026-09-29T12:00:00Z"),
            ] {
                let mut conflicting = newer.clone();
                let project = conflicting.project.as_mut().unwrap();
                project.revision = revision.map(str::to_string);
                project.initiative_ids = vec!["different-wave".into()];
                let result = fixture
                    .store
                    .put_pm_task(&repo.to_string_lossy(), "linear", conflicting, None, None)
                    .await;
                if revision != Some("2026-09-29T11:00:00Z") {
                    assert!(result.is_err());
                } else {
                    result.unwrap();
                }
                let evidence = fixture
                    .store
                    .pm_task_observation(&repo.to_string_lossy(), "linear", "FIX-1")
                    .await
                    .unwrap();
                assert_eq!(evidence.record, Some(stored.clone()));
                assert_eq!(
                    evidence.state,
                    if revision != Some("2026-09-29T11:00:00Z") {
                        crate::store::PlanningState::Invalid
                    } else {
                        crate::store::PlanningState::Available
                    }
                );
            }
            // Replaying old membership cannot clear the explicit uncertainty.
            fixture
                .store
                .put_pm_snapshot(list.clone(), None)
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
                .projects
                .is_empty());
            fixture
                .store
                .confirm_pm_project_archival(
                    &repo.to_string_lossy(),
                    "linear",
                    newer.project.unwrap(),
                    original.observed_at,
                )
                .await
                .unwrap();
            list.snapshot.projects.clear();
            list.snapshot.items.clear();
            fixture.store.put_pm_snapshot(list, None).await.unwrap();
            assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
        })
        .await;
}

#[tokio::test]
async fn cold_detail_resolves_configured_wave_before_projecting() {
    for ownership in ["same", "foreign", "unmapped"] {
        let fixture = Fixture::new().await;
        let (repo, wave) = fixture.planning_repo().await;
        fixture.seed(now() + 3600).await;
        let mut refreshed = project();
        refreshed["name"] = "Refreshed plan".into();
        refreshed["updatedAt"] = "2026-09-30T11:00:00Z".into();
        if ownership != "same" {
            refreshed["initiatives"]["nodes"][0]["id"] = "elsewhere".into();
        }
        if ownership == "foreign" {
            std::fs::create_dir_all(repo.join("wave/other")).unwrap();
            std::fs::write(
                repo.join("wave/other/GOAL.md"),
                "---\npm:\n  linear_initiative: elsewhere\n---\nOther work.\n",
            )
            .unwrap();
            fixture
                .store
                .create_wave(&crate::work::wave::Wave::new(
                    WaveId::new(),
                    "other".into(),
                    repo.to_string_lossy().into_owned(),
                ))
                .await
                .unwrap();
        }
        let (url, _) = spawn(vec![
            team_response(),
            json_response(StatusCode::OK, issue(project())),
            json_response(StatusCode::OK, issue(project())),
            team_response(),
            json_response(StatusCode::OK, issue(refreshed.clone())),
            json_response(StatusCode::OK, issue(refreshed)),
        ])
        .await;
        PM_TEST_CONTEXT
            .scope(fixture.context(&url), async {
                read_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                    .await
                    .unwrap();
                let original = fixture
                    .store
                    .list_projects(Some(wave.id()))
                    .await
                    .unwrap()
                    .remove(0);
                // Evict normalized provider facts, retaining durable work as on a cold detail read.
                let conn = rusqlite::Connection::open(&fixture.database).unwrap();
                conn.execute_batch(
                    "DELETE FROM pm_wave_projects; DELETE FROM pm_items; DELETE FROM pm_projects;",
                )
                .unwrap();
                drop(conn);
                let result = inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                    .await
                    .unwrap();
                let current = fixture
                    .store
                    .get_project(&original.id)
                    .await
                    .unwrap()
                    .unwrap();
                if ownership == "same" {
                    assert!(result.refresh_error.is_none(), "{:?}", result.refresh_error);
                    assert_eq!(current.id, original.id);
                    assert_eq!(current.wave_id, original.wave_id);
                    assert_eq!(current.plan.name, "Refreshed plan");
                } else {
                    assert!(result.refresh_error.is_some());
                    assert_eq!(current, original);
                    assert_eq!(
                        fixture.store.list_projects(None).await.unwrap(),
                        vec![original]
                    );
                }
                assert!(fixture
                    .store
                    .pm_snapshot(wave.id())
                    .await
                    .unwrap()
                    .is_none());
            })
            .await;
    }
}

#[tokio::test]
async fn cancelled_detail_acceptance_keeps_wave_excluded_until_commit() {
    cancelled_acceptance(true).await;
}

#[tokio::test]
async fn cancelled_snapshot_acceptance_keeps_wave_excluded_until_commit() {
    cancelled_acceptance(false).await;
}

async fn cancelled_acceptance(detail: bool) {
    let fixture = Fixture::new().await;
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let mut responses = if detail {
        vec![
            team_response(),
            json_response(StatusCode::OK, issue(project())),
            json_response(StatusCode::OK, issue(project())),
        ]
    } else {
        vec![
            team_response(),
            json_response(
                StatusCode::OK,
                json!({"data":{"initiative":{"projects":{
                    "nodes":[project()],"pageInfo":{"hasNextPage":false,"endCursor":null}
                }}}}),
            ),
            json_response(
                StatusCode::OK,
                json!({"data":{"project":{"issues":{
                    "nodes":[issue(project())["data"]["issue"]],"pageInfo":{"hasNextPage":false,"endCursor":null}
                }}}}),
            ),
        ]
    };
    responses.extend([
        json_response(StatusCode::OK, json!({"data":{"initiative":{"projects":{
            "nodes":[project()],"pageInfo":{"hasNextPage":false,"endCursor":null}
        }}}})),
        json_response(StatusCode::OK, json!({"data":{"project":{"issues":{
            "nodes":[issue(project())["data"]["issue"]],"pageInfo":{"hasNextPage":false,"endCursor":null}
        }}}})),
    ]);
    let (url, _) = spawn(responses).await;
    let reteam = super::ResolvedReteamContext {
        repository: super::RepositoryPmContext {
            client: crate::pm::linear::LinearClient::with_base_url(
                "fixture".into(),
                Some("team-1".into()),
                url.clone(),
            ),
            provider: crate::pm::PmProviderKind::Linear,
            repo_id: crate::repository::RepoId::parse("loopflowstudio/fixture").unwrap(),
            team_id: "team-1".into(),
        },
        team_key: "FIX".into(),
        store: crate::store::Store::from_sqlite_for_test(fixture.store.sqlite.clone()),
    };
    let entered = Arc::new(tokio::sync::Notify::new());
    let (release, blocked) = std::sync::mpsc::channel();
    let context = fixture.context(&url);
    let write_repo = repo.clone();
    let gate = (entered.clone(), Arc::new(std::sync::Mutex::new(blocked)));
    let writer = tokio::spawn(PM_TEST_CONTEXT.scope(
        context,
        crate::store::PLANNING_ACCEPTANCE_GATE.scope(gate, async move {
            if detail {
                inspect_task_planning_async(&write_repo, "FIX-1", PmRefresh::Force)
                    .await
                    .map(|_| ())
            } else {
                let ctx = super::resolve_context(&write_repo, "product").await?;
                super::refresh_pm_snapshot(&write_repo, "product", &ctx)
                    .await
                    .map(|_| ())
            }
        }),
    ));
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    writer.abort();
    assert!(writer.await.unwrap_err().is_cancelled());
    let excluded = PM_TEST_CONTEXT
        .scope(
            fixture.context(&url),
            tokio::time::timeout(
                std::time::Duration::from_millis(100),
                super::apply_or_plan_repository_reteam(
                    &reteam,
                    &repo,
                    false,
                    &crate::ops::NullProgress,
                ),
            ),
        )
        .await
        .is_err();
    release.send(()).unwrap();
    let recovered = PM_TEST_CONTEXT
        .scope(
            fixture.context(&url),
            super::apply_or_plan_repository_reteam(
                &reteam,
                &repo,
                false,
                &crate::ops::NullProgress,
            ),
        )
        .await
        .unwrap();
    assert!(recovered.moves.is_empty());
    let accepted = fixture
        .store
        .pm_task_observation(&repo.to_string_lossy(), "linear", "FIX-1")
        .await
        .unwrap();
    assert!(
        accepted.record.is_some(),
        "cancelled caller's worker did not finish acceptance"
    );
    assert!(
        excluded,
        "a competing operation acquired the Wave before acceptance committed"
    );
}

#[tokio::test]
async fn cold_detail_rechecks_team_after_acquiring_wave() {
    let fixture = Fixture::new().await;
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let mut discovered = issue(project());
    discovered["data"]["issue"]["team"]["id"] = "old-team".into();
    discovered["data"]["issue"]["project"]["teams"] = json!({"nodes":[{"id":"old-team"}]});
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, discovered),
        json_response(StatusCode::OK, issue(project())),
    ])
    .await;
    let record = PM_TEST_CONTEXT
        .scope(
            fixture.context(&url),
            read_task_planning_async(&repo, "FIX-1", PmRefresh::Force),
        )
        .await
        .unwrap();
    assert_eq!(record.item.team_id, "team-1");
    assert_eq!(record.project.unwrap().team_ids, ["team-1"]);
}

#[tokio::test]
async fn delayed_absence_preserves_a_newer_accepted_task() {
    let fixture = Fixture::new().await;
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let mut absent = json_response(StatusCode::OK, json!({"data":{"issue":null}}));
    absent.gate = Some((entered.clone(), release.clone()));
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        json_response(StatusCode::OK, issue(project())),
        team_response(),
        absent,
        json_response(StatusCode::OK, json!({"data":{"issue":null}})),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let mut confirmed = read_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .unwrap();
            confirmed.item.revision = Some("2026-10-05T12:00:00Z".into());
            confirmed.item.identifier = "FIX-2".into();
            let (inspection, ()) = tokio::join!(
                inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Force),
                async {
                    entered.wait().await;
                    fixture
                        .store
                        .put_pm_task(
                            &repo.to_string_lossy(),
                            "linear",
                            confirmed.clone(),
                            None,
                            None,
                        )
                        .await
                        .unwrap();
                    release.wait().await;
                }
            );
            // Stable-ID reobservation still finds a renamed Task after the requested identifier changes.
            let stored = fixture
                .store
                .pm_task_observation(&repo.to_string_lossy(), "linear", "issue-1")
                .await
                .unwrap();
            assert_eq!(stored.state, crate::store::PlanningState::Available);
            assert_eq!(stored.record.unwrap().item.identifier, "FIX-2");
            let inspection = inspection.unwrap();
            assert!(
                inspection.refresh_error.is_none(),
                "{:?}",
                inspection.refresh_error
            );
            assert_eq!(
                inspection.observation.state,
                crate::store::PlanningState::Available
            );
            assert_eq!(
                inspection.observation.record.unwrap().item.identifier,
                "FIX-2"
            );
        })
        .await;
}
