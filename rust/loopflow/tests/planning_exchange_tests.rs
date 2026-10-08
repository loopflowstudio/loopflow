use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use loopflow::engine::planning_exchange::{
    PlanningChangeId, PlanningComment, PlanningDisposition, PlanningField, PlanningMembership,
    PlanningSnapshot, PortableTask,
};
use loopflow::engine::planning_git::{PlanningGit, PlanningPublication};
use loopflow_test_support::TestRepo;

fn change(id: &str) -> PlanningChangeId {
    PlanningChangeId::new(id.into()).unwrap()
}

fn task() -> PortableTask {
    PortableTask {
        title: PlanningField::new(change("creation"), "Original title".into()),
        brief: PlanningField::new(change("creation"), "Original brief".into()),
        membership: PlanningField::new(
            change("creation"),
            PlanningMembership {
                wave_id: "wave-infrastructure".into(),
                project_id: Some("project-chapter".into()),
            },
        ),
        issue: PlanningField::new(change("creation"), None),
        disposition: PlanningField::new(change("creation"), PlanningDisposition::Open),
        comments: BTreeMap::new(),
    }
}

fn snapshot(task: PortableTask) -> PlanningSnapshot {
    PlanningSnapshot {
        tasks: BTreeMap::from([("task-existing-id".into(), task)]),
    }
}

fn comment(task: &mut PortableTask, id: &str, body: &str) {
    task.comments
        .entry(id.into())
        .or_default()
        .insert(PlanningComment {
            author: Some("Jack Heart".into()),
            body: body.into(),
        });
}

fn complete(task: &mut PortableTask, id: &str) {
    task.disposition
        .write(
            change(id),
            PlanningDisposition::Completed {
                summary: "Delivered".into(),
            },
        )
        .unwrap();
}

#[test]
fn independent_fields_comments_and_creations_combine_without_minting_ids() {
    let mut laptop = task();
    let mut worker = laptop.clone();
    laptop
        .title
        .write(change("title-edit"), "New title".into())
        .unwrap();
    worker
        .brief
        .write(change("brief-edit"), "New brief".into())
        .unwrap();
    comment(&mut laptop, "laptop-comment", "Laptop direction");
    comment(&mut worker, "worker-comment", "Worker finding");
    let left = snapshot(laptop);
    let mut right = snapshot(worker);
    right.tasks.insert("follow-up-id".into(), task());
    // Divergent historical identities remain distinct even with matching contents.
    right.tasks.insert("legacy-id".into(), task());
    let merged = left.merge(&right);
    let task = &merged.tasks["task-existing-id"];
    assert_eq!(task.title.resolved().unwrap(), "New title");
    assert_eq!(task.brief.resolved().unwrap(), "New brief");
    assert_eq!(task.comments.len(), 2);
    assert_eq!(merged.tasks.len(), 3);
    assert_eq!(merged, right.merge(&left));
    assert_eq!(merged, merged.merge(&left).merge(&right));
    assert_eq!(
        merged,
        PlanningSnapshot::from_bytes(&merged.to_bytes().unwrap()).unwrap()
    );
}

#[test]
fn contested_disposition_preserves_both_inputs_without_blocking_comments() {
    let mut laptop = task();
    let mut worker = laptop.clone();
    complete(&mut laptop, "complete");
    worker
        .disposition
        .write(
            change("cancel"),
            PlanningDisposition::Abandoned {
                reason: "Direction changed".into(),
            },
        )
        .unwrap();
    comment(
        &mut worker,
        "finding",
        "Keep this even while disposition conflicts",
    );
    let merged = laptop.merge(&worker);
    assert_eq!(merged.disposition.resolved(), None);
    assert_eq!(merged.disposition.values().len(), 2);
    assert_eq!(merged.comments["finding"].len(), 1);
    let mut resolved = merged.clone();
    resolved
        .disposition
        .write(change("resolve"), PlanningDisposition::Open)
        .unwrap();
    assert_eq!(resolved.merge(&worker).merge(&laptop), resolved);
}

#[test]
fn observed_reopening_survives_delayed_completion_and_exchange_order() {
    let initial = task();
    let mut completed = initial.clone();
    complete(&mut completed, "complete");
    let mut reopened = completed.clone();
    reopened
        .disposition
        .write(change("reopen"), PlanningDisposition::Open)
        .unwrap();
    let mut unrelated = initial.clone();
    comment(&mut unrelated, "delayed-comment", "Still useful");
    let result = reopened.merge(&completed).merge(&unrelated);
    assert_eq!(
        result.disposition.resolved(),
        Some(&PlanningDisposition::Open)
    );
    assert_eq!(result, reopened.merge(&completed.merge(&unrelated)));
    assert_eq!(result, unrelated.merge(&reopened).merge(&completed));
    assert_eq!(result.comments.len(), 1);
    // An unobserved reopening is concurrent even though its value equals the base.
    let mut independent_reopen = initial;
    independent_reopen
        .disposition
        .write(change("independent-reopen"), PlanningDisposition::Open)
        .unwrap();
    assert_eq!(
        completed.merge(&independent_reopen).disposition.resolved(),
        None
    );
}

#[test]
fn omission_never_deletes_and_explicit_deletion_keeps_task_contents() {
    let mut original = task();
    comment(&mut original, "comment", "Retained history");
    let mut deleted = original.clone();
    deleted
        .disposition
        .write(change("delete"), PlanningDisposition::Deleted)
        .unwrap();
    let omitted = PlanningSnapshot {
        tasks: BTreeMap::new(),
    };
    let original = snapshot(original);
    assert_eq!(original.merge(&omitted), original);
    let deleted = snapshot(deleted);
    let result = deleted.merge(&original).merge(&omitted);
    assert_eq!(result, deleted);
    let retained = &result.tasks["task-existing-id"];
    assert_eq!(retained.title.resolved().unwrap(), "Original title");
    assert_eq!(retained.comments.len(), 1);
    assert_eq!(
        retained.disposition.resolved(),
        Some(&PlanningDisposition::Deleted)
    );
}

#[test]
fn immutable_comment_identity_deduplicates_replay_and_retains_conflicting_bytes() {
    let mut left = task();
    comment(&mut left, "same-comment", "First content");
    let mut right = task();
    comment(&mut right, "same-comment", "Conflicting content");
    assert_eq!(left.merge(&left), left);
    let merged = left.merge(&right);
    assert_eq!(merged.comments["same-comment"].len(), 2);
    assert_eq!(merged.merge(&left).merge(&right), merged);
}

#[test]
fn same_value_concurrent_writes_keep_both_causes_until_resolution() {
    let original = task();
    let mut left = original.clone();
    let mut right = original;
    complete(&mut left, "left-complete");
    complete(&mut right, "right-complete");
    let mut merged = left.merge(&right);
    assert!(merged.disposition.resolved().is_some());
    merged
        .disposition
        .write(change("reopen"), PlanningDisposition::Open)
        .unwrap();
    assert_eq!(merged.merge(&left).merge(&right), merged);
    assert!(merged
        .disposition
        .write(change("left-complete"), PlanningDisposition::Open)
        .is_err());
}

#[test]
fn membership_conflict_never_invents_a_wave_project_pair() {
    let mut left = task();
    let mut right = left.clone();
    let a = PlanningMembership {
        wave_id: "wave-a".into(),
        project_id: Some("project-a".into()),
    };
    let b = PlanningMembership {
        wave_id: "wave-b".into(),
        project_id: Some("project-b".into()),
    };
    left.membership.write(change("move-a"), a.clone()).unwrap();
    right.membership.write(change("move-b"), b.clone()).unwrap();
    assert_eq!(
        left.merge(&right).membership.values(),
        BTreeSet::from([&a, &b])
    );
}

#[test]
fn portable_decode_rejects_execution_fields_and_invalid_causal_context() {
    let bytes = snapshot(task()).to_bytes().unwrap();
    for field in [
        "worktree",
        "workflow",
        "sessions",
        "processes",
        "claims",
        "controls",
    ] {
        let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        json["tasks"]["task-existing-id"][field] = serde_json::json!("machine-local");
        assert!(PlanningSnapshot::from_bytes(&serde_json::to_vec(&json).unwrap()).is_err());
    }
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["tasks"]["task-existing-id"]["title"]["retired"] = serde_json::json!(["creation"]);
    assert!(PlanningSnapshot::from_bytes(&serde_json::to_vec(&json).unwrap()).is_err());
}

#[test]
fn causal_conflicts_and_resolution_survive_git_publication_and_fresh_repository() {
    let first = TestRepo::new();
    first.push();
    let origin = Command::new("git")
        .current_dir(first.path())
        .args(["remote", "get-url", "origin"])
        .output()
        .unwrap();
    assert!(origin.status.success());
    let origin = String::from_utf8(origin.stdout).unwrap();
    let second = TestRepo::new();
    let laptop = PlanningGit::new(first.path(), origin.trim()).unwrap();
    let worker = PlanningGit::new(second.path(), origin.trim()).unwrap();
    let initial = snapshot(task());
    let base = laptop
        .save(&initial.to_bytes().unwrap(), None, None)
        .unwrap();
    assert_eq!(
        laptop.publish(&base.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    let fetched = worker.fetch().unwrap().unwrap();
    let mut left = initial;
    let mut right = PlanningSnapshot::from_bytes(&fetched.bytes).unwrap();
    complete(left.tasks.get_mut("task-existing-id").unwrap(), "complete");
    let worker_task = right.tasks.get_mut("task-existing-id").unwrap();
    worker_task
        .disposition
        .write(
            change("cancel"),
            PlanningDisposition::Abandoned {
                reason: "Changed".into(),
            },
        )
        .unwrap();
    comment(worker_task, "comment", "Keep while conflicted");
    let left_revision = laptop
        .save(&left.to_bytes().unwrap(), Some(&base.revision), None)
        .unwrap();
    let right_revision = worker
        .save(&right.to_bytes().unwrap(), None, Some(&fetched.revision))
        .unwrap();
    assert_eq!(
        laptop.publish(&left_revision.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    let PlanningPublication::Pending {
        remote: Some(remote),
    } = worker.publish(&right_revision.revision).unwrap()
    else {
        panic!("stale publication must retain both inputs");
    };
    let merged = right.merge(&PlanningSnapshot::from_bytes(&remote.bytes).unwrap());
    assert_eq!(
        merged.tasks["task-existing-id"].disposition.resolved(),
        None
    );
    let revision = worker
        .save(
            &merged.to_bytes().unwrap(),
            Some(&right_revision.revision),
            Some(&remote.revision),
        )
        .unwrap();
    assert_eq!(
        worker.publish(&revision.revision).unwrap(),
        PlanningPublication::Confirmed
    );
    // A fresh repository has no SQLite checkpoint or remembered merge base.
    let fresh = TestRepo::new();
    let reader = PlanningGit::new(fresh.path(), origin.trim()).unwrap();
    let recovered = reader.fetch().unwrap().unwrap();
    let mut recovered = PlanningSnapshot::from_bytes(&recovered.bytes).unwrap();
    assert_eq!(recovered, merged);
    recovered
        .tasks
        .get_mut("task-existing-id")
        .unwrap()
        .disposition
        .write(change("reopen"), PlanningDisposition::Open)
        .unwrap();
    assert_eq!(recovered.merge(&left).merge(&right), recovered);
    assert_eq!(recovered.tasks["task-existing-id"].comments.len(), 1);
}
