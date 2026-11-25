//! Lost replies and conflicts on the Linear stand-in, end to end through the
//! queue: a save, a send whose reply is lost, and the look that confirms it.

use std::path::Path;

use super::{mark_issue_updated, with_planning_task};
use crate::store::Store;
use loopflow_storage::sqlite::tracker_actions::{
    ActionKind, ActionOwner, ActionPhase, TrackerAction,
};
use loopflow_storage::work::task::{Task, TaskUpdate};

async fn deliver(store: &Store, repo: &Path, task: &Task) {
    let owner = ActionOwner::Task(task.id.clone());
    crate::ops::tracker::drain::drain_repository(
        store,
        &repo.to_string_lossy(),
        crate::ops::tracker::drain::Scope::Only(&owner),
        crate::ops::tracker::drain::Bound::NOW,
    )
    .await
    .unwrap();
}

fn rename(store: &Store, task: &Task, title: &str) {
    let current = store.sqlite.task(&task.id).unwrap().unwrap();
    store
        .sqlite
        .edit_task(
            &task.id,
            current.revision,
            &TaskUpdate {
                title: Some(title.into()),
                ..Default::default()
            },
        )
        .unwrap();
}

fn action(store: &Store, task: &Task, kind: ActionKind) -> TrackerAction {
    store
        .sqlite
        .tracker_actions_for(&ActionOwner::Task(task.id.clone()))
        .unwrap()
        .into_iter()
        .rfind(|action| action.kind == kind)
        .unwrap()
}

#[test]
fn a_lost_field_reply_is_found_by_one_look_and_not_sent_again() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            state.lock().await.field_reply_lost = true;
            rename(&fixture.store, task, "Renamed");
            deliver(&fixture.store, repo, task).await;
            let sent = action(&fixture.store, task, ActionKind::Field);
            assert_eq!(sent.phase, ActionPhase::Sent);
            state.lock().await.field_reads_blocked = false;
            deliver(&fixture.store, repo, task).await;
            assert_eq!(state.lock().await.field_writes, 1);
            let resolved = action(&fixture.store, task, ActionKind::Field);
            assert_eq!(resolved.phase, ActionPhase::Confirmed);
            assert_eq!(resolved.conflict, None);
        });
    });
}

#[test]
fn a_coworkers_newer_edit_wins_and_the_losing_value_stays_visible() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            state.lock().await.field_reply_lost = true;
            rename(&fixture.store, task, "Mine");
            deliver(&fixture.store, repo, task).await;
            {
                let mut provider = state.lock().await;
                provider.field_reads_blocked = false;
                provider.issues[0]["title"] = serde_json::json!("Theirs");
                mark_issue_updated(&mut provider.issues[0]);
            }
            deliver(&fixture.store, repo, task).await;
            assert_eq!(state.lock().await.field_writes, 1);
            let resolved = action(&fixture.store, task, ActionKind::Field);
            assert_eq!(resolved.phase, ActionPhase::Conflicted);
            assert_eq!(resolved.value["value"], "Mine");
            assert_eq!(resolved.conflict.unwrap()["value"]["value"], "Theirs");
            let saved = fixture.store.sqlite.task(&task.id).unwrap().unwrap();
            assert_eq!(saved.title, "Theirs");
        });
    });
}

#[test]
fn a_deletion_restored_in_linear_after_a_lost_reply_retires() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            state.lock().await.lose_deletion_reply = true;
            fixture.store.sqlite.delete_task(&task.id).unwrap();
            deliver(&fixture.store, repo, task).await;
            assert_eq!(
                action(&fixture.store, task, ActionKind::Delete).phase,
                ActionPhase::Sent
            );
            {
                let mut provider = state.lock().await;
                provider.issues[0]["trashed"] = serde_json::Value::Null;
                mark_issue_updated(&mut provider.issues[0]);
            }
            deliver(&fixture.store, repo, task).await;
            assert_eq!(state.lock().await.deletion_writes, 1);
            assert_eq!(
                action(&fixture.store, task, ActionKind::Delete).phase,
                ActionPhase::Retired
            );
        });
    });
}

#[test]
fn a_released_queue_delivers_after_its_chapter_projects_collapse() {
    use crate::ops::tracker::creation_id;
    use loopflow_model::durable::{RepositoryId, TaskId};
    use loopflow_storage::migration_runner::{apply_before_migration, upgrade_from_migration};
    use loopflow_storage::test_fixture::{build, observers};
    use rusqlite::params;
    use serde_json::json;

    with_planning_task(|runtime, fixture, repo, _, state| {
        let database = fixture.directory.path().join("released.db");
        let conn = rusqlite::Connection::open(&database).unwrap();
        apply_before_migration(&observers(), &build(), &conn, "remove_chapters");
        let repository = RepositoryId::new();
        conn.execute(
            "INSERT INTO repository_plans(id,repo,selected) VALUES(?1,?2,1)",
            params![repository.as_str(), repo.to_str().unwrap()],
        )
        .unwrap();
        conn.execute_batch(include_str!(
            "../../../../loopflow-storage/tests/fixtures/migrations/chapter_deliveries.sql"
        ))
        .unwrap();
        let [queued, arrived, moving, moved, retry] =
            ["Queued", "Arrived", "Moving", "Moved", "Retry"].map(|title| {
                TaskId::from_raw(
                    conn.query_row(
                        "SELECT id FROM tasks WHERE issue_title=?1",
                        [title],
                        |row| row.get::<_, String>(0),
                    )
                    .unwrap(),
                )
            });
        upgrade_from_migration(&observers(), &build(), &conn, "remove_chapters");
        drop(conn);
        let store =
            Store::from_sqlite_for_test(loopflow_storage::test_fixture::open(&database).unwrap());
        let arrived_id = creation_id(arrived.as_str()).unwrap();
        {
            let mut linear = runtime.block_on(state.lock());
            linear.extra_projects.extend(
                ["november", "december", "refunds"]
                    .map(crate::test_support::planning_tasks::planning_project),
            );
            for (index, (id, project)) in [
                (arrived_id.as_str(), "november"),
                ("moving", "november"),
                ("moved", "november"),
                ("retry", "refunds"),
            ]
            .into_iter()
            .enumerate()
            {
                let mut issue = linear.issues[0].clone();
                issue["id"] = json!(id);
                issue["identifier"] = json!(format!("FIX-{}", index + 20));
                issue["project"]["id"] = json!(project);
                linear.issues.push(issue);
            }
        }
        for id in [&queued, &arrived, &moving, &moved, &retry] {
            let task = store.sqlite.task(id).unwrap().unwrap();
            runtime.block_on(deliver(&store, repo, &task));
            let actions = store
                .sqlite
                .tracker_actions_for(&ActionOwner::Task(id.clone()))
                .unwrap();
            assert_eq!(actions[0].phase, ActionPhase::Confirmed, "{actions:?}");
        }
        assert_eq!(
            store
                .sqlite
                .task(&arrived)
                .unwrap()
                .unwrap()
                .tracker_id
                .unwrap()
                .as_str(),
            arrived_id
        );
        let linear = runtime.block_on(state.lock());
        assert_eq!(linear.creation_writes, 1);
        assert_eq!(linear.field_writes, 2);
        for (id, project) in [
            (creation_id(queued.as_str()).unwrap(), "december"),
            (arrived_id, "november"),
            ("moving".into(), "refunds"),
            ("moved".into(), "november"),
            ("retry".into(), "december"),
        ] {
            let issue = linear
                .issues
                .iter()
                .find(|issue| issue["id"] == id)
                .unwrap();
            assert_eq!(issue["project"]["id"], project);
        }
    });
}
