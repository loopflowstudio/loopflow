use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use super::{mark_issue_updated, seed_provider_task, with_planning_task, PlanningState};
use crate::durable::WorkRef;
use crate::ops::pm::test_fixture::Fixture;
use crate::ops::pm::{load_show_snapshot, PmRefresh};
use crate::ops::NullProgress;
use crate::pm::PmItemUpdate;
use crate::store::Store;
use crate::work::task::Task;

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
            load_show_snapshot(repo, "product", PmRefresh::Force, &NullProgress)
                .await
                .unwrap();
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
            current.plan.revision,
            &PmItemUpdate {
                rank: Some(rank),
                ..Default::default()
            },
        )
        .unwrap();
}

fn local_order(store: &Store, tasks: &[Task]) -> Vec<String> {
    let mut ordered = tasks
        .iter()
        .map(|t| {
            store
                .sqlite
                .planning_task(&t.id)
                .unwrap()
                .record
                .unwrap()
                .item
        })
        .collect::<Vec<_>>();
    ordered.sort_by_key(|item| item.rank);
    ordered.into_iter().map(|i| i.id).collect()
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
fn planning_order_active_connection_delivers_after_outage() {
    with_order(|runtime, fixture, repo, tasks, state| {
        runtime.block_on(async {
            state.lock().await.field_outage = true;
            let sync = crate::ops::linear_observe::PlanningSync::start(
                fixture.store.clone(),
                repo.to_string_lossy().into_owned(),
            )
            .unwrap();
            reorder(&fixture.store, &tasks[3], 0);
            let receipt = fixture
                .store
                .sqlite
                .pending_project_changes(&tasks[0].project_id)
                .unwrap()[0]
                .clone();
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            tokio::time::timeout(Duration::from_secs(8), async {
                loop {
                    let failed: bool = conn
                        .query_row(
                            "SELECT error IS NOT NULL FROM project_changes WHERE id=?1",
                            [&receipt.id],
                            |r| r.get(0),
                        )
                        .unwrap();
                    if failed {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                fixture
                    .store
                    .sqlite
                    .pending_project_changes(&tasks[0].project_id)
                    .unwrap()[0]
                    .id,
                receipt.id
            );
            state.lock().await.field_outage = false;
            tokio::time::timeout(Duration::from_secs(8), async {
                while fixture
                    .store
                    .sqlite
                    .project_order_delivery(&tasks[0].project_id)
                    .unwrap()
                    .is_some()
                {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                remote_order(&state).await,
                ["issue-4", "issue-1", "issue-2", "issue-3"]
            );
            assert_eq!(
                local_order(&fixture.store, &tasks),
                remote_order(&state).await
            );
            assert_no_conflicts(fixture);
            assert!(tasks.iter().all(|task| fixture
                .store
                .sqlite
                .workflow(&task.id)
                .unwrap()
                .is_none()));
            drop(sync);
        });
    });
}

#[test]
fn planning_order_peers_recover_lost_reply_and_preserve_later_save() {
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
            let destination = crate::engine::planning_git::PlanningDestination::new(
                "/synthetic/order-remote", "refs/loopflow/planning/shared/order-test").unwrap();
            let repo_name = repo.to_string_lossy();
            let destination_id = fixture.store.sqlite.bind_peer_planning(&repo_name, &destination).unwrap();
            fixture.store.sqlite.select_peer_waves(&repo_name, &destination_id,
                std::slice::from_ref(&tasks[0].wave_id)).unwrap();
            let home = tempfile::tempdir().unwrap();
            let database = home.path().join("peer.db");
            let reopened = crate::store::open_ephemeral_store(
                &crate::store::StorageConfig::sqlite(database.clone()),
            ).await.unwrap();
            reopened.sqlite.bind_peer_planning(&repo_name, &destination).unwrap();
            let exchange = |source: &Store, target: &Store| {
                let snapshot = source.sqlite.export_peer_planning(&repo_name, &destination_id).unwrap();
                target.sqlite.import_peer_planning(&repo_name, &destination_id, "fixture", &snapshot).unwrap();
                assert!(target.sqlite.peer_planning_status(&repo_name).unwrap()[0].conflicts.is_empty());
            };
            exchange(&fixture.store, &reopened);
            // The peer imported the exact attempt, not just the rendered ranks.
            let conn = rusqlite::Connection::open(&database).unwrap();
            let receipt_body = |connection: &rusqlite::Connection| -> (String, Option<String>, String, bool, bool) {
                connection.query_row("SELECT value_json,base_json,order_effects_json,attempted,acknowledged FROM project_changes WHERE id=?1",
                    [&receipt], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).unwrap()
            };
            let source_conn = rusqlite::Connection::open(&fixture.database).unwrap();
            assert_eq!(receipt_body(&conn), receipt_body(&source_conn));
            assert!(!reopened.sqlite.put_pm_project_order(&repo_name,
                fixture.store.get_project(&tasks[0].project_id).await.unwrap().unwrap().plan.linear_id().unwrap().as_str(), &[]).unwrap());
            assert_eq!(receipt_body(&conn), receipt_body(&source_conn));
            assert!(deliver(&reopened, repo, &tasks[0]).await.is_err());
            assert_eq!(state.lock().await.field_writes, 1);
            // A later local save must survive the older effect's confirmation,
            // even when those receipts arrive in a different local sequence.
            reorder(&reopened, &tasks[1], 0);
            let later = reopened.sqlite.pending_project_changes(&tasks[0].project_id).unwrap()
                .into_iter().find(|change| change.field == "task_order").unwrap();
            assert_ne!(later.id, receipt);
            let journal = reopened.sqlite.export_peer_planning(&repo_name, &destination_id).unwrap();
            exchange(&reopened, &fixture.store);
            exchange(&fixture.store, &reopened);
            assert_eq!(reopened.sqlite.export_peer_planning(&repo_name, &destination_id).unwrap(), journal);
            assert_eq!(
                reopened
                    .sqlite
                    .project_order_delivery(&tasks[0].project_id)
                    .unwrap()
                    .unwrap()
                    .id,
                receipt
            );
            drop(reopened);
            let reopened = crate::store::open_ephemeral_store(
                &crate::store::StorageConfig::sqlite(database.clone()),
            ).await.unwrap();
            state.lock().await.field_reads_blocked = false;
            deliver(&reopened, repo, &tasks[0]).await.unwrap();
            assert_eq!(
                remote_order(&state).await,
                ["issue-2", "issue-3", "issue-4", "issue-1"]
            );
            assert_eq!(state.lock().await.field_writes, 3);
            exchange(&reopened, &fixture.store);
            assert_eq!(local_order(&fixture.store, &tasks), remote_order(&state).await);
            assert!(fixture.store.sqlite.project_order_delivery(&tasks[0].project_id).unwrap().is_none());
            assert_eq!(receipt_body(&conn), receipt_body(&source_conn));
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            assert_eq!(state.lock().await.field_writes, 3);
            // Both the superseded desired order and the later save remain recoverable.
            assert_eq!(conn.query_row("SELECT value_json FROM project_changes WHERE id=?1", [&later.id],
                |row| row.get::<_, String>(0)).unwrap(), later.value.to_string());
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
                let receipt = fixture
                    .store
                    .sqlite
                    .pending_project_changes(&tasks[0].project_id)
                    .unwrap()
                    .into_iter()
                    .find(|c| c.field == "task_order")
                    .unwrap();
                {
                    let mut provider = state.lock().await;
                    provider.issues[0]["title"] = json!("Inbound title during reorder");
                    mark_issue_updated(&mut provider.issues[0]);
                }
                load_show_snapshot(repo, "product", PmRefresh::Force, &NullProgress)
                    .await
                    .unwrap();
                assert_eq!(
                    local_order(&fixture.store, &tasks),
                    ["issue-3", "issue-4", "issue-1", "issue-2"]
                );
                assert_eq!(
                    fixture
                        .store
                        .sqlite
                        .planning_task(&tasks[0].id)
                        .unwrap()
                        .record
                        .unwrap()
                        .item
                        .name,
                    "Inbound title during reorder"
                );
                assert_eq!(
                    fixture
                        .store
                        .sqlite
                        .pending_project_changes(&tasks[0].project_id)
                        .unwrap()
                        .into_iter()
                        .find(|c| c.field == "task_order")
                        .unwrap()
                        .id,
                    receipt.id
                );
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
            result.unwrap();
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
            let receipt = fixture
                .store
                .sqlite
                .pending_project_changes(&tasks[0].project_id)
                .unwrap()
                .remove(0);
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
                    [&receipt.id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
                receipt.value
            );
            assert_ne!(
                serde_json::from_str::<serde_json::Value>(&conflict).unwrap()["value"],
                receipt.value
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
            load_show_snapshot(repo, "product", PmRefresh::Force, &NullProgress)
                .await
                .unwrap();
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
            let stale = fixture
                .store
                .sqlite
                .pm_snapshot(&tasks[0].wave_id)
                .unwrap()
                .unwrap();
            reorder(&fixture.store, &tasks[3], 0);
            deliver(&fixture.store, repo, &tasks[0]).await.unwrap();
            let expected = remote_order(&state).await;
            fixture.store.sqlite.put_pm_snapshot(&stale).unwrap();
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
                .edit_project(&tasks[0].project_id, Some("Independent title"), None)
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
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].field, "task_order");
            assert_no_conflicts(fixture);
        });
    });
}
