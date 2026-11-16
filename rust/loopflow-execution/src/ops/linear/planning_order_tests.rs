use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use super::{mark_issue_updated, seed_provider_task, with_planning_task, PlanningState};
use crate::ops::linear::refresh_wave;
use crate::ops::linear::test_fixture::Fixture;
use crate::store::Store;
use loopflow_storage::sqlite::tracker_effects::{
    EffectKind, EffectOwner, EffectPhase, TrackerEffect,
};
use loopflow_storage::work::task::Task;
use loopflow_storage::work::task::TaskUpdate;

fn with_order(
    test: impl FnOnce(
        &tokio::runtime::Runtime,
        &Fixture,
        &Path,
        Vec<Task>,
        Arc<tokio::sync::Mutex<PlanningState>>,
    ),
) {
    with_planning_task(|runtime, fixture, repo, first, state| {
        let mut tasks = vec![first.clone()];
        for name in ["B", "C", "D"] {
            let item = seed_provider_task(runtime, &state, repo, name, "Keep notes").unwrap();
            tasks.push(
                runtime
                    .block_on(fixture.store.get_task_by_issue(&item.id))
                    .unwrap()
                    .unwrap(),
            );
        }
        runtime.block_on(async {
            let mut provider = state.lock().await;
            for (index, issue) in provider.issues.iter_mut().enumerate() {
                issue["prioritySortOrder"] = json!((index / 2) as f64 * 10.0);
                issue["sortOrder"] = json!((index % 2) as f64 * 10.0);
                mark_issue_updated(issue);
            }
            drop(provider);
            refresh_wave(repo, "product").await.unwrap();
        });
        test(runtime, fixture, repo, tasks, state);
    });
}

fn reorder(store: &Store, task: &Task, rank: u32) {
    let current = store.sqlite.task(&task.id).unwrap().unwrap();
    store
        .sqlite
        .edit_task(
            &task.id,
            current.revision,
            &TaskUpdate {
                rank: Some(rank),
                ..Default::default()
            },
        )
        .unwrap();
}

fn local_order(store: &Store, tasks: &[Task]) -> Vec<String> {
    let mut ordered = tasks
        .iter()
        .map(|t| store.sqlite.task(&t.id).unwrap().unwrap())
        .collect::<Vec<_>>();
    ordered.sort_by_key(|item| item.rank);
    ordered
        .into_iter()
        .map(|i| i.linear_id.unwrap().as_str().to_owned())
        .collect()
}

async fn remote_order(state: &Arc<tokio::sync::Mutex<PlanningState>>) -> Vec<String> {
    let mut issues = state.lock().await.issues.clone();
    issues.sort_by(|a, b| {
        a["prioritySortOrder"]
            .as_f64()
            .unwrap()
            .total_cmp(&b["prioritySortOrder"].as_f64().unwrap())
            .then_with(|| {
                a["sortOrder"]
                    .as_f64()
                    .unwrap()
                    .total_cmp(&b["sortOrder"].as_f64().unwrap())
            })
    });
    issues
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

async fn deliver(store: &Store, repo: &Path, task: &Task) -> crate::ops::OpsResult<()> {
    let owner = EffectOwner::Project(task.project_id.clone().unwrap());
    crate::ops::tracker::drain::drain_repository(
        store,
        &repo.to_string_lossy(),
        crate::ops::tracker::drain::Scope::Only(&owner),
        crate::ops::tracker::drain::Bound::NOW,
    )
    .await
    .map(|_| ())
}

/// The Project's unsettled order effect.
fn order_effect(
    store: &Store,
    project: &loopflow_model::durable::ProjectId,
) -> Option<TrackerEffect> {
    store
        .sqlite
        .tracker_effects_for(&EffectOwner::Project(project.clone()))
        .unwrap()
        .into_iter()
        .rfind(|effect| {
            effect.kind == EffectKind::Order
                && matches!(effect.phase, EffectPhase::Pending | EffectPhase::Attempted)
        })
}

fn assert_no_conflicts(fixture: &Fixture) {
    let conn = rusqlite::Connection::open(&fixture.database).unwrap();
    let conflicts: i64 = conn
        .query_row(
            "SELECT count(*) FROM tracker_effects WHERE kind='order' AND conflict_json IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(conflicts, 0);
}

#[test]
fn planning_order_equal_keys_preserve_groups_through_intermediate_moves() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            {
                let mut provider = state.lock().await;
                for issue in &mut provider.issues {
                    issue["prioritySortOrder"] = json!(7.0);
                    issue["sortOrder"] = json!(0.0);
                    mark_issue_updated(issue);
                }
            }
            refresh_wave(repo, "product").await.unwrap();
            reorder(&fixture.store, &tasks[3], 1);
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            assert_eq!(
                remote_order(&state).await,
                ["issue-1", "issue-4", "issue-2", "issue-3"]
            );
            assert!(state
                .lock()
                .await
                .issues
                .iter()
                .all(|i| i["prioritySortOrder"] == 7.0));
            assert_no_conflicts(fixture);
        });
    });
}

#[test]
fn planning_order_new_member_survives_pending_reorder() {
    with_order(|runtime, fixture, repo, tasks, state| {
        reorder(&fixture.store, &tasks[3], 0);
        let new =
            seed_provider_task(runtime, &state, repo, "New member", "Do not lose this").unwrap();
        runtime.block_on(async {
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            let ordered = remote_order(&state).await;
            assert!(ordered.contains(&new.id));
            assert_eq!(
                ordered
                    .iter()
                    .filter(|id| *id != &new.id)
                    .cloned()
                    .collect::<Vec<_>>(),
                ["issue-4", "issue-1", "issue-2", "issue-3"]
            );
            assert!(order_effect(&fixture.store, tasks[0].project_id.as_ref().unwrap()).is_none());
            assert_no_conflicts(fixture);
        });
    });
}

#[test]
fn a_saved_reorder_reaches_linear_and_settles() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            reorder(&fixture.store, &tasks[3], 0);
            let project = tasks[0].project_id.as_ref().unwrap();
            assert_eq!(
                order_effect(&fixture.store, project).unwrap().phase,
                EffectPhase::Pending
            );
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            assert_eq!(
                local_order(&fixture.store, &tasks),
                remote_order(&state).await
            );
            assert!(order_effect(&fixture.store, project).is_none());
            assert_no_conflicts(fixture);
        });
    });
}

#[test]
fn a_lost_order_reply_is_looked_for_and_not_repeated() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            reorder(&fixture.store, &tasks[3], 0);
            let project = tasks[0].project_id.as_ref().unwrap();
            state.lock().await.field_reply_lost = true;
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            assert_eq!(
                order_effect(&fixture.store, project).unwrap().phase,
                EffectPhase::Attempted
            );
            let writes = state.lock().await.field_writes;
            state.lock().await.field_reads_blocked = false;
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            assert_eq!(
                state.lock().await.field_writes,
                writes,
                "the move landed, so nothing is sent again"
            );
            assert!(order_effect(&fixture.store, project).is_none());
            assert_eq!(
                local_order(&fixture.store, &tasks),
                remote_order(&state).await
            );
            assert_no_conflicts(fixture);
        });
    });
}
