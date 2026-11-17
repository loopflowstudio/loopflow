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
