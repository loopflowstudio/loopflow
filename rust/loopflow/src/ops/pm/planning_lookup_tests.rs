#![allow(clippy::await_holding_lock)] // Serialize process-wide Machine selection for each async fixture.

use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::json;
use tokio::sync::Barrier;

use super::test_fixture::refresh_task_input;
use super::test_fixture::{now, Fixture, PlanningEnvironment};
use super::{inspect_task_planning_async, PmRefresh, PM_TEST_CONTEXT};
use crate::id::WaveId;
use crate::pm::test_server::{json_response, spawn, QueuedResponse};
use crate::pm::PmSnapshot;
use crate::store::{PmSnapshotRow, PmTaskRecord};

fn issue(project: serde_json::Value) -> serde_json::Value {
    json!({"data":{"issue":{
        "id":"issue-1", "identifier":"FIX-1", "url":null, "branchName":"dev/fix-1-existing",
        "title":"Inspect a planning-only Task", "description":"Keep this work visible",
        "completedAt": null, "dueDate": null, "prioritySortOrder":0.0, "sortOrder":0.0, "updatedAt":"2026-09-29T12:00:00.123Z", "assignee":null,
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
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, wave) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let mut provider_project = project();
    provider_project["name"] = "Summer work — customer requests".into();
    let mut provider_issue = issue(provider_project);
    provider_issue["data"]["issue"]["dueDate"] = "2026-10-15".into();
    provider_issue["data"]["issue"]["assignee"] = json!({"id":"assignee-1"});
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, provider_issue.clone()),
        json_response(StatusCode::OK, provider_issue.clone()),
        team_response(),
        json_response(StatusCode::OK, provider_issue.clone()),
        json_response(StatusCode::OK, provider_issue),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let record = refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            assert_eq!(record.item.name, "Inspect a planning-only Task");
            assert_eq!(
                record.project.as_ref().unwrap().name,
                "Summer work — customer requests"
            );
            assert_eq!(
                record.item.branch_name.as_deref(),
                Some("dev/fix-1-existing")
            );
            let resolved = crate::ops::pm::acquire_task(&repo, "issue-1", PmRefresh::Never)
                .await
                .unwrap();
            assert_eq!(resolved.due_date.as_deref(), Some("2026-10-15"));
            assert_eq!(resolved.assignee.as_deref(), Some("assignee-1"));
            assert_eq!(resolved.state.as_deref(), Some("unstarted"));
            assert!(!resolved.completed);
            assert!(resolved.completed_at.is_none());
            assert_eq!(resolved.title, record.item.name);
            assert_eq!(resolved.description, record.item.description);
            assert_eq!(
                resolved.linear_id.as_ref().unwrap().as_str(),
                record.item.id
            );
            assert_eq!(
                fixture.store.get_task(&resolved.id).await.unwrap(),
                Some(resolved)
            );
            for task in fixture.store.list_tasks(None).await.unwrap() {
                assert!(task.worktree.is_none());
                assert!(fixture.store.task_prs(&task.id).await.unwrap().is_empty());
                assert!(!fixture.store.task_started(&task.id).await.unwrap());
            }
            assert_eq!(fixture.store.list_projects(None).await.unwrap().len(), 1);
            assert_eq!(fixture.wave_refresh_at(&wave), None);

            // A list carries membership, not another copy of the Task's title.
            let mut snapshot = snapshot(wave.id(), &record);
            fixture
                .store
                .put_pm_snapshot(snapshot.clone(), None)
                .await
                .unwrap();
            let refreshed = refresh_task_input(&repo, "FIX-1", PmRefresh::Force)
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
                    .get_task_by_issue("FIX-1")
                    .await
                    .unwrap()
                    .unwrap()
                    .title,
                edited.item.name
            );
            snapshot.snapshot.items.clear();
            snapshot.synced_at += 2;
            fixture.store.put_pm_snapshot(snapshot, None).await.unwrap();
            assert_eq!(
                refresh_task_input(&repo, "fix-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                edited
            );
            assert_eq!(
                fixture
                    .store
                    .list_tasks(Some(wave.id()))
                    .await
                    .unwrap()
                    .len(),
                1
            );
        })
        .await;
}

#[tokio::test]
async fn acquisition_returns_saved_task_fields_without_replacing_pending_edits() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        json_response(StatusCode::OK, issue(project())),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let task = super::acquire_task(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            let saved = fixture
                .store
                .sqlite
                .edit_task(
                    &task.id,
                    task.revision,
                    &crate::pm::PmItemUpdate {
                        name: Some("Saved during disconnection".into()),
                        assignee: Some(Some("pending-assignee".into())),
                        ..Default::default()
                    },
                )
                .unwrap();
            let pending = fixture.store.sqlite.pending_task_changes(&task.id).unwrap();
            let acquired = super::acquire_task(&repo, "issue-1", PmRefresh::Never)
                .await
                .unwrap();
            assert_eq!(acquired, saved);
            assert_eq!(acquired.assignee.as_deref(), Some("pending-assignee"));
            assert_eq!(acquired.title, "Saved during disconnection");
            assert_eq!(
                fixture.store.sqlite.pending_task_changes(&task.id).unwrap(),
                pending
            );
            assert_eq!(acquired.worktree, task.worktree);
            assert_eq!(acquired.branch, task.branch);
            assert_eq!(acquired.parent_pr_id, task.parent_pr_id);
            assert!(fixture.store.task_prs(&task.id).await.unwrap().is_empty());
            assert!(!fixture.store.task_started(&task.id).await.unwrap());
        })
        .await;
}

#[tokio::test]
async fn rejected_project_snapshot_preserves_durable_project_facts() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, _) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(json!(null))),
    ])
    .await;
    PM_TEST_CONTEXT
        .scope(fixture.context(&url), async {
            let record = refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            assert!(record.project.is_none());
            assert!(record.item.project_id.is_none());
            let error = crate::ops::pm::acquire_task(&repo, "FIX-1", PmRefresh::Never)
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
        let _lock = crate::journal::test_env_lock();
        let _restore = PlanningEnvironment::isolate();
        let fixture = Fixture::new().await;
        std::env::set_var("LF_HOME", fixture.directory.path());
        let (repo, _) = fixture.planning_repo().await;
        fixture.seed(now() + 3600).await;
        let (url, _) = spawn(vec![
            team_response(),
            json_response(StatusCode::OK, response),
        ])
        .await;
        PM_TEST_CONTEXT
            .scope(fixture.context(&url), async {
                let error = refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
                    .await
                    .unwrap_err();
                assert!(error.to_string().contains(diagnostic), "{error}");
                assert!(fixture
                    .store
                    .linear_issue_evidence(&repo.to_string_lossy(), "FIX-1")
                    .await
                    .unwrap()
                    .issue_id
                    .is_none());
                assert!(fixture.store.list_tasks(None).await.unwrap().is_empty());
            })
            .await;
    }
}

#[tokio::test]
async fn failed_refresh_preserves_the_last_observation_and_its_age() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
            let original = refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            let inspection = inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .unwrap();
            assert_eq!(
                inspection.observation.state,
                crate::store::PlanningState::Unavailable
            );
            assert_eq!(
                inspection.observation.observed_at,
                Some(original.observed_at)
            );
            assert_eq!(fixture.retained_task(&repo, "issue-1"), original);
            assert!(inspection
                .refresh_error
                .unwrap()
                .contains("provider unavailable"));
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Force)
                .await
                .is_err());
            assert_eq!(
                refresh_task_input(&repo, "issue-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                original
            );
        })
        .await;
}

#[tokio::test]
async fn omitted_detail_fields_do_not_clear_known_planning() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
            let original = refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            for field in missing_fields {
                let field = field.strip_prefix("project.").unwrap_or(field);
                let error = refresh_task_input(&repo, "FIX-1", PmRefresh::Force)
                    .await
                    .unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains(&format!("missing field `{field}`")),
                    "{error}"
                );
                assert_eq!(
                    refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
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
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
            let original = refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Force)
                .await
                .is_err());
            fixture
                .store
                .put_pm_snapshot(snapshot(wave.id(), &original), None)
                .await
                .unwrap();
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            assert_ne!(
                fixture
                    .store
                    .linear_issue_evidence(&repo.to_string_lossy(), "FIX-1")
                    .await
                    .unwrap()
                    .state,
                crate::store::PlanningState::Available
            );
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
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
            assert_eq!(
                read.observation.state,
                crate::store::PlanningState::Unavailable
            );
            assert!(read.refresh_error.unwrap().contains("provider unavailable"));
            assert_eq!(read.observation.observed_at, Some(record.observed_at));
            assert_eq!(fixture.retained_task(&repo, "issue-1"), record);
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap_err()
                .to_string()
                .contains("provider unavailable"));
            assert_eq!(
                refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                record
            );
        })
        .await;
}

#[tokio::test]
async fn provider_revisions_and_change_receipts_converge_without_execution() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
            let original = refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
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
                refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                    .await
                    .unwrap(),
                confirmed
            );
            assert_eq!(
                fixture
                    .store
                    .get_task_by_issue("FIX-1")
                    .await
                    .unwrap()
                    .unwrap()
                    .title,
                confirmed.item.name
            );
            let mut conflicting = confirmed.clone();
            conflicting.item.name = "Contradiction at the same revision".into();
            assert!(fixture
                .store
                .put_pm_task(&scope, "linear", conflicting, None, None)
                .await
                .is_err());
            assert_eq!(
                refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
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
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            assert_ne!(
                fixture
                    .store
                    .linear_issue_evidence(&repo.to_string_lossy(), "FIX-1")
                    .await
                    .unwrap()
                    .state,
                crate::store::PlanningState::Available
            );
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
                refresh_task_input(&repo, "issue-1", PmRefresh::Never)
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
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            fixture
                .store
                .put_pm_task(&scope, "linear", confirmed.clone(), None, None)
                .await
                .unwrap();
            assert_eq!(
                refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
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
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            assert_ne!(
                fixture
                    .store
                    .linear_issue_evidence(&repo.to_string_lossy(), "FIX-1")
                    .await
                    .unwrap()
                    .state,
                crate::store::PlanningState::Available
            );
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
            assert!(refresh_task_input(&repo, "FIX-2", PmRefresh::Never)
                .await
                .is_err());
            for task in fixture.store.list_tasks(None).await.unwrap() {
                assert!(task.worktree.is_none());
                assert!(fixture.store.task_prs(&task.id).await.unwrap().is_empty());
                assert!(!fixture.store.task_started(&task.id).await.unwrap());
            }
        })
        .await;
}

#[tokio::test]
async fn inspection_retains_invalid_removed_and_absent_facts_without_admitting_work() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
            let original = refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
                .await
                .unwrap();
            fixture
                .store
                .invalidate_linear_issue(
                    &repo.to_string_lossy(),
                    fixture
                        .store
                        .linear_issue_evidence(&repo.to_string_lossy(), "issue-1")
                        .await
                        .unwrap(),
                    None,
                )
                .await
                .unwrap();
            let invalid = inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Never)
                .await
                .unwrap();
            assert_eq!(
                invalid.observation.state,
                crate::store::PlanningState::Invalid
            );
            assert_eq!(invalid.observation.observed_at, Some(original.observed_at));
            assert_eq!(fixture.retained_task(&repo, "issue-1"), original);
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            let absent = inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Force)
                .await
                .unwrap();
            assert_eq!(
                absent.observation.state,
                crate::store::PlanningState::Absent
            );
            assert_eq!(absent.observation.observed_at, Some(original.observed_at));
            assert_eq!(fixture.retained_task(&repo, "issue-1"), original);
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
            assert_eq!(removed.observation.observed_at, Some(original.observed_at));
            assert_eq!(fixture.retained_task(&repo, "issue-1"), original);
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            for task in fixture.store.list_tasks(None).await.unwrap() {
                assert!(task.worktree.is_none());
                assert!(fixture.store.task_prs(&task.id).await.unwrap().is_empty());
                assert!(!fixture.store.task_started(&task.id).await.unwrap());
            }
        })
        .await;
}

#[tokio::test]
async fn removal_during_absent_lookup_preserves_confirmed_evidence() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
            let original = refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
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
            assert_eq!(
                inspection.observation.observed_at,
                Some(original.observed_at)
            );
            assert_eq!(fixture.retained_task(&repo, "issue-1"), original);
            assert!(inspection.refresh_error.is_none());
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            for task in fixture.store.list_tasks(None).await.unwrap() {
                assert!(task.worktree.is_none());
                assert!(fixture.store.task_prs(&task.id).await.unwrap().is_empty());
                assert!(!fixture.store.task_started(&task.id).await.unwrap());
            }
        })
        .await;
}

#[tokio::test]
async fn project_revisions_order_shared_facts_and_unordered_membership_stays_unresolved() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
            let original = refresh_task_input(&repo, "FIX-1", PmRefresh::Auto)
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
            let stored = refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                .await
                .unwrap();
            assert_eq!(stored.project, newer.project);
            assert_eq!(stored.observed_at, list.synced_at);
            let saved = fixture
                .store
                .get_task_by_issue("FIX-1")
                .await
                .unwrap()
                .unwrap();
            let saved_project = fixture
                .store
                .get_project(&saved.project_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(saved_project.name, "Renamed chapter");
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
                fixture
                    .store
                    .get_task_by_issue("FIX-1")
                    .await
                    .unwrap()
                    .unwrap(),
                saved
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
                    .linear_issue_evidence(&repo.to_string_lossy(), "FIX-1")
                    .await
                    .unwrap();
                assert_eq!(evidence.observed_at, Some(stored.observed_at));
                assert_eq!(fixture.retained_task(&repo, "issue-1"), stored);
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
            assert!(refresh_task_input(&repo, "FIX-1", PmRefresh::Never)
                .await
                .is_err());
            assert_ne!(
                fixture
                    .store
                    .linear_issue_evidence(&repo.to_string_lossy(), "FIX-1")
                    .await
                    .unwrap()
                    .state,
                crate::store::PlanningState::Available
            );
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
            for task in fixture.store.list_tasks(None).await.unwrap() {
                assert!(task.worktree.is_none());
                assert!(fixture.store.task_prs(&task.id).await.unwrap().is_empty());
                assert!(!fixture.store.task_started(&task.id).await.unwrap());
            }
        })
        .await;
}

#[tokio::test]
async fn cold_detail_resolves_configured_wave_before_projecting() {
    for ownership in ["same", "foreign", "unmapped"] {
        let _lock = crate::journal::test_env_lock();
        let _restore = PlanningEnvironment::isolate();
        let fixture = Fixture::new().await;
        std::env::set_var("LF_HOME", fixture.directory.path());
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
            fixture
                .store
                .sqlite
                .ensure_wave(repo.to_str().unwrap(), "other")
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
                refresh_task_input(&repo, "FIX-1", PmRefresh::Force)
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
                    assert_eq!(current.name, "Refreshed plan");
                } else {
                    assert!(result.refresh_error.is_some());
                    assert_eq!(current, original);
                    assert_eq!(
                        fixture.store.list_projects(None).await.unwrap(),
                        vec![original]
                    );
                }
                assert_eq!(fixture.wave_refresh_at(&wave), None);
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
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
        .linear_issue_evidence(&repo.to_string_lossy(), "FIX-1")
        .await
        .unwrap();
    assert!(
        accepted.issue_id.is_some(),
        "cancelled caller's worker did not finish acceptance"
    );
    assert!(
        excluded,
        "a competing operation acquired the Wave before acceptance committed"
    );
}

#[tokio::test]
async fn cold_detail_rechecks_team_after_acquiring_wave() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
            refresh_task_input(&repo, "FIX-1", PmRefresh::Force),
        )
        .await
        .unwrap();
    assert_eq!(record.item.team_id.as_deref(), Some("team-1"));
    assert_eq!(record.project.unwrap().team_ids, ["team-1"]);
}

#[tokio::test]
async fn delayed_absence_preserves_a_newer_accepted_task() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
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
            let mut confirmed = refresh_task_input(&repo, "FIX-1", PmRefresh::Force)
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
                .linear_issue_evidence(&repo.to_string_lossy(), "issue-1")
                .await
                .unwrap();
            assert_eq!(stored.state, crate::store::PlanningState::Available);
            assert_eq!(
                fixture.retained_task(&repo, "issue-1").item.identifier,
                "FIX-2"
            );
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
            assert_eq!(inspection.observation.issue_id.as_deref(), Some("issue-1"));
        })
        .await;
}

#[tokio::test]
async fn acquisition_reads_ownership_without_reconstructing_provider_content() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let fixture = Fixture::new().await;
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, wave) = fixture.planning_repo().await;
    fixture.seed(now() + 3600).await;
    let (url, _) = spawn(vec![
        team_response(),
        json_response(StatusCode::OK, issue(project())),
        json_response(StatusCode::OK, issue(project())),
    ])
    .await;
    PM_TEST_CONTEXT.scope(fixture.context(&url), async {
        let task = super::acquire_task(&repo, "FIX-1", PmRefresh::Auto).await.unwrap();
        let before = fixture.store.linear_issue_evidence(&repo.to_string_lossy(), "FIX-1").await.unwrap();
        let project_before = fixture.store.get_project(&task.project_id).await.unwrap();
        let wave_age = fixture.wave_refresh_at(&wave);
        let conn = rusqlite::Connection::open(&fixture.database).unwrap();
        conn.execute("UPDATE pm_items SET body=json_set(body,'$.name',json('[]'),'$.rank','unrelated bad content') WHERE id='issue-1'", []).unwrap();
        conn.execute("UPDATE pm_projects SET body=json_set(body,'$.name',json('[]'),'$.krs','unrelated bad content') WHERE id='project-1'", []).unwrap();
        assert_eq!(super::acquire_task(&repo, "issue-1", PmRefresh::Never).await.unwrap(), task);
        assert_eq!(fixture.store.linear_issue_evidence(&repo.to_string_lossy(), "FIX-1").await.unwrap(), before);
        for column in ["membership_unresolved", "archived"] {
            conn.execute(&format!("UPDATE pm_projects SET {column}=1 WHERE id='project-1'"), []).unwrap();
            assert!(super::acquire_task(&repo, "FIX-1", PmRefresh::Never).await.is_err());
            let rejected = inspect_task_planning_async(&repo, "FIX-1", PmRefresh::Never).await.unwrap();
            assert_eq!(rejected.observation.state, crate::store::PlanningState::Invalid);
            assert_eq!(rejected.observation.observed_at, before.observed_at);
            conn.execute(&format!("UPDATE pm_projects SET {column}=0 WHERE id='project-1'"), []).unwrap();
        }
        // A change receipt invalidates admission even when unrelated content is unreadable.
        fixture.store.observe_pm_issue_change("issue-1", None, false).await.unwrap();
        let invalidated = fixture.store.linear_issue_evidence(&repo.to_string_lossy(), "FIX-1").await.unwrap();
        assert_eq!(invalidated.state, crate::store::PlanningState::Invalid);
        assert_eq!(invalidated.observed_at, before.observed_at);
        assert_eq!(fixture.store.get_task(&task.id).await.unwrap(), Some(task));
        assert_eq!(fixture.store.get_project(&project_before.as_ref().unwrap().id).await.unwrap(), project_before);
        assert_eq!(fixture.wave_refresh_at(&wave), wave_age);
    }).await;
}
