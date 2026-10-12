#[cfg(test)]
use crate::test_linear_input::SeedLinearSqlite;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use super::{mark_issue_updated, seed_provider_task, with_planning_task, PlanningState};
use crate::durable::WorkRef;
use crate::ops::linear::refresh_wave;
use crate::ops::linear::test_fixture::Fixture;
use crate::store::Store;
use crate::work::task::Task;
use crate::work::task::TaskUpdate;

fn saved_order(fixture: &Fixture, task: &Task) -> (String, serde_json::Value) {
    let conn = rusqlite::Connection::open(&fixture.database).unwrap();
    let (id,value):(String,String)=conn.query_row("SELECT id,value_json FROM project_changes WHERE project_id=?1 AND field='task_order' ORDER BY seq DESC LIMIT 1", [task.project_id.as_str()], |r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    (id, serde_json::from_str(&value).unwrap())
}

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
    crate::ops::planning_delivery::sync_fields(
        store,
        repo,
        &WorkRef::Project(task.project_id.clone()),
    )
    .await
}

fn assert_no_conflicts(fixture: &Fixture) {
    let conn = rusqlite::Connection::open(&fixture.database).unwrap();
    let conflicts: i64 = conn.query_row("SELECT count(*) FROM project_changes WHERE field='task_order' AND conflict_json IS NOT NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(conflicts, 0);
}

#[test]
fn planning_order_lost_reply_recovers_partial_reorder_after_reopen() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            reorder(&fixture.store, &tasks[3], 0);
            reorder(&fixture.store, &tasks[2], 0);
            let receipt = fixture
                .store
                .sqlite
                .project_order_delivery(&tasks[0].project_id)
                .unwrap()
                .unwrap()
                .id;
            state.lock().await.field_reply_lost = true;
            assert!(deliver(&fixture.store, repo, &tasks[0]).await.is_err());
            assert_eq!(state.lock().await.field_writes, 1);
            assert!(deliver(&fixture.store, repo, &tasks[0]).await.is_err());
            assert_eq!(state.lock().await.field_writes, 1);
            let reopened = crate::store::open_ephemeral_store(
                &crate::store::StorageConfig::sqlite(fixture.database.clone()),
            )
            .await
            .unwrap();
            assert_eq!(
                reopened
                    .sqlite
                    .project_order_delivery(&tasks[0].project_id)
                    .unwrap()
                    .unwrap()
                    .id,
                receipt
            );
            state.lock().await.field_reads_blocked = false;
            deliver(&reopened, repo, &tasks[0]).await.unwrap();
            assert_eq!(
                remote_order(&state).await,
                ["issue-3", "issue-4", "issue-1", "issue-2"]
            );
            assert_eq!(state.lock().await.field_writes, 2);
            assert_eq!(local_order(&reopened, &tasks), remote_order(&state).await);
            assert!(reopened
                .sqlite
                .project_order_delivery(&tasks[0].project_id)
                .unwrap()
                .is_none());
            assert_no_conflicts(fixture);
        });
    });
}

#[test]
fn planning_order_acquisition_during_write_preserves_newer_save_and_scalar_edits() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            reorder(&fixture.store, &tasks[3], 0);
            let arrived = Arc::new(tokio::sync::Notify::new());
            let release = Arc::new(tokio::sync::Notify::new());
            {
                let mut provider = state.lock().await;
                provider.field_arrived = Some(arrived.clone());
                provider.field_release = Some(release.clone());
            }
            let delivery = deliver(&fixture.store, repo, &tasks[0]);
            let concurrent = async {
                arrived.notified().await;
                reorder(&fixture.store, &tasks[2], 0);
                let receipt = saved_order(fixture, &tasks[0]);
                {
                    let mut provider = state.lock().await;
                    provider.issues[0]["title"] = json!("Inbound title during reorder");
                    mark_issue_updated(&mut provider.issues[0]);
                }
                refresh_wave(repo, "product").await.unwrap();
                assert_eq!(
                    local_order(&fixture.store, &tasks),
                    ["issue-3", "issue-4", "issue-1", "issue-2"]
                );
                assert_eq!(
                    fixture
                        .store
                        .sqlite
                        .task(&tasks[0].id)
                        .unwrap()
                        .unwrap()
                        .title,
                    "Inbound title during reorder"
                );
                assert_eq!(saved_order(fixture, &tasks[0]).0, receipt.0);
                assert_no_conflicts(fixture);
                // Another delivery connection cannot duplicate the in-flight move.
                deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
                release.notify_one();
            };
            let (result, ()) = tokio::time::timeout(Duration::from_secs(4), async {
                tokio::join!(delivery, concurrent)
            })
            .await
            .unwrap();
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("older Project order"));
            // The acknowledged mutation does not trigger a readback. A new
            // delivery boundary obtains the newer external state before writing.
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            assert_eq!(
                remote_order(&state).await,
                ["issue-3", "issue-4", "issue-1", "issue-2"]
            );
            assert_eq!(state.lock().await.field_writes, 2);
            assert_no_conflicts(fixture);
        });
    });
}

#[test]
fn planning_order_observed_conflict_adopts_linear_and_retains_losing_list() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            reorder(&fixture.store, &tasks[3], 0);
            let receipt = saved_order(fixture, &tasks[0]);
            {
                let mut provider = state.lock().await;
                provider.issues[2]["prioritySortOrder"] = json!(-10.0);
                mark_issue_updated(&mut provider.issues[2]);
            }
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            assert_eq!(
                remote_order(&state).await,
                local_order(&fixture.store, &tasks)
            );
            assert_eq!(state.lock().await.field_writes, 0);
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            let (saved, conflict): (String, String) = conn
                .query_row(
                    "SELECT value_json,conflict_json FROM project_changes WHERE id=?1",
                    [&receipt.0],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
                receipt.1
            );
            assert_ne!(
                serde_json::from_str::<serde_json::Value>(&conflict).unwrap()["value"],
                receipt.1
            );
        });
    });
}

#[test]
fn planning_order_unconfirmed_move_is_not_repeated() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            reorder(&fixture.store, &tasks[3], 0);
            state.lock().await.field_reject_write = true;
            assert!(deliver(&fixture.store, repo, &tasks[0]).await.is_err());
            state.lock().await.field_reject_write = false;
            assert!(deliver(&fixture.store, repo, &tasks[0])
                .await
                .unwrap_err()
                .to_string()
                .contains("uncertain"));
            assert_eq!(state.lock().await.field_writes, 1);
            assert_eq!(
                local_order(&fixture.store, &tasks),
                ["issue-4", "issue-1", "issue-2", "issue-3"]
            );
            assert_no_conflicts(fixture);
        });
    });
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
fn planning_order_stale_list_cannot_undo_confirmed_reorder() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            let (ctx, initiative) = crate::ops::linear::resolve_context(repo, "product")
                .await
                .unwrap();
            let stale = crate::test_linear_input::InventoryFixture {
                wave_id: tasks[0].wave_id.clone(),
                provider: "linear".into(),
                initiative: initiative.clone(),
                synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                snapshot: crate::ops::linear::test_fixture::capture_inventory(
                    repo,
                    "product",
                    &ctx,
                    &initiative,
                    &fixture.store,
                )
                .await
                .unwrap(),
            };
            reorder(&fixture.store, &tasks[3], 0);
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            let expected = remote_order(&state).await;
            fixture.store.sqlite.seed_inventory(&stale).unwrap();
            assert_eq!(local_order(&fixture.store, &tasks), expected);
            assert_no_conflicts(fixture);
        });
    });
}

#[test]
fn planning_order_confirmed_removal_does_not_strand_saved_reorder() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            reorder(&fixture.store, &tasks[3], 0);

            state.lock().await.issues.retain(|i| i["id"] != "issue-2");
            assert!(deliver(&fixture.store, repo, &tasks[0]).await.is_err());
            assert_eq!(
                local_order(&fixture.store, &tasks),
                ["issue-4", "issue-1", "issue-2", "issue-3"]
            );
            assert_eq!(state.lock().await.field_writes, 0);
            fixture.store.sqlite.delete_task(&tasks[1].id).unwrap();
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            assert_eq!(
                remote_order(&state).await,
                ["issue-4", "issue-1", "issue-3"]
            );
            assert!(fixture
                .store
                .sqlite
                .project_order_delivery(&tasks[0].project_id)
                .unwrap()
                .is_none());
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
            assert!(fixture
                .store
                .sqlite
                .project_order_delivery(&tasks[0].project_id)
                .unwrap()
                .is_none());
            assert_no_conflicts(fixture);
        });
    });
}

#[test]
fn planning_order_uncertainty_does_not_stall_project_field_delivery() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            reorder(&fixture.store, &tasks[3], 0);
            state.lock().await.field_reject_write = true;
            assert!(deliver(&fixture.store, repo, &tasks[0]).await.is_err());
            state.lock().await.field_reject_write = false;
            fixture
                .store
                .sqlite
                .edit_project(&tasks[0].project_id, Some("Independent title"), None, None)
                .unwrap();
            assert!(deliver(&fixture.store, repo, &tasks[0]).await.is_err());
            assert_eq!(
                state.lock().await.project_fields.as_ref().unwrap()["name"],
                "Independent title"
            );
            let pending = fixture
                .store
                .sqlite
                .pending_project_changes(&tasks[0].project_id)
                .unwrap();
            assert!(pending.is_empty());
            assert!(fixture
                .store
                .sqlite
                .project_order_delivery(&tasks[0].project_id)
                .unwrap()
                .is_some());
            assert_no_conflicts(fixture);
        });
    });
}

#[test]
fn linear_rate_limit_retries_rejected_order_move() {
    with_order(|runtime, fixture, repo, tasks, state| {
        reorder(&fixture.store, &tasks[3], 0);
        let work = WorkRef::Project(tasks[0].project_id.clone());
        runtime.block_on(async {
            state.lock().await.rate_limit_query = Some("mutation DeliverTaskField");
            assert!(
                crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                    .await
                    .is_err()
            );
            let receipt = fixture
                .store
                .sqlite
                .project_order_delivery(&tasks[0].project_id)
                .unwrap()
                .unwrap();
            assert!(
                receipt.effects.is_empty(),
                "a rejected move has no uncertain effect"
            );
            tokio::time::sleep(Duration::from_millis(2100)).await;
            crate::ops::planning_delivery::sync_fields(&fixture.store, repo, &work)
                .await
                .unwrap();
            assert!(fixture
                .store
                .sqlite
                .project_order_delivery(&tasks[0].project_id)
                .unwrap()
                .is_none());
            assert_eq!(
                local_order(&fixture.store, &tasks),
                remote_order(&state).await
            );
        });
    });
}
