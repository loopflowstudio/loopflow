use std::collections::{BTreeMap, BTreeSet};

use loopflow::planning_exchange::{PlanningMutation, PlanningSnapshot};
use loopflow::work::edit::{Disposition, WorkEdit, WorkValue};
use serde_json::json;

fn edit(field: &str, value: &str) -> WorkEdit {
    WorkEdit {
        target: "task-existing".into(),
        change: match field {
            "task_title" => WorkValue::TaskTitle(Some(value.into())),
            "task_description" => WorkValue::TaskDescription(Some(value.into())),
            "disposition" => WorkValue::Disposition(Disposition {
                planning_state: Some(value.into()),
                planning_completed: 0,
                planning_completed_at: None,
            }),
            _ => panic!("unknown test field"),
        },
    }
}

fn write(snapshot: &mut PlanningSnapshot, id: &str, field: &str, value: &str, clock: i64) {
    let edit = edit(field, value);
    let parents = snapshot
        .heads()
        .filter(|(_, change)| {
            change.edit.target == edit.target && change.edit.change.field() == edit.change.field()
        })
        .map(|(id, _)| id.to_owned())
        .collect();
    snapshot.changes.insert(
        id.into(),
        PlanningMutation {
            edit,
            clock,
            parents,
        },
    );
}

fn value(snapshot: &PlanningSnapshot, field: &str) -> serde_json::Value {
    let value = snapshot
        .winners()
        .find(|(_, change)| change.edit.change.field() == field)
        .unwrap()
        .1
        .edit
        .change
        .value()
        .unwrap();
    if field == "disposition" {
        value["planning_state"].clone()
    } else {
        value
    }
}

#[test]
fn concurrent_edits_converge_without_losing_independent_fields_or_losing_values() {
    let mut base = PlanningSnapshot::default();
    write(&mut base, "initial", "task_title", "Initial", 1);
    let mut laptop = base.clone();
    let mut worker = base;
    write(&mut laptop, "left", "task_title", "Laptop", 2);
    write(&mut worker, "right", "task_title", "Worker", 3);
    write(&mut worker, "brief", "task_description", "Independent", 4);
    let merged = laptop.merge(&worker).unwrap();
    assert_eq!(merged, worker.merge(&laptop).unwrap());
    assert_eq!(merged.merge(&laptop).unwrap(), merged);
    assert_eq!(value(&merged, "task_title"), "Worker");
    assert_eq!(value(&merged, "task_description"), "Independent");
    assert_eq!(
        merged.changes["left"].edit.change.value().unwrap(),
        "Laptop"
    );
    assert_eq!(
        PlanningSnapshot::from_bytes(&merged.to_bytes().unwrap()).unwrap(),
        merged
    );
}

#[test]
fn observed_reopening_beats_delayed_completion() {
    let mut completed = PlanningSnapshot::default();
    write(&mut completed, "complete", "disposition", "completed", 1);
    let mut reopened = completed.clone();
    write(&mut reopened, "reopen", "disposition", "unstarted", 2);
    assert_eq!(
        value(&completed.merge(&reopened).unwrap(), "disposition"),
        "unstarted"
    );
    assert_eq!(
        value(&reopened.merge(&completed).unwrap(), "disposition"),
        "unstarted"
    );
}

#[test]
fn equal_values_retain_both_causes_and_ties_use_stable_identity() {
    let mut left = PlanningSnapshot::default();
    let mut right = PlanningSnapshot::default();
    write(&mut left, "a", "task_title", "Same", 1);
    write(&mut right, "b", "task_title", "Same", 1);
    let mut merged = left.merge(&right).unwrap();
    write(&mut merged, "c", "task_title", "Next", 2);
    assert_eq!(
        merged.changes["c"].parents,
        BTreeSet::from(["a".into(), "b".into()])
    );
    assert_eq!(value(&merged.merge(&left).unwrap(), "task_title"), "Next");
    right.changes.get_mut("b").unwrap().edit = edit("task_title", "Tie winner");
    assert_eq!(
        value(&left.merge(&right).unwrap(), "task_title"),
        "Tie winner"
    );
}

#[test]
fn conflicting_identity_missing_ancestors_and_execution_fields_are_rejected() {
    let mut saved = PlanningSnapshot::default();
    write(&mut saved, "a", "task_title", "Saved", 1);
    let mut reused = saved.clone();
    reused.changes.get_mut("a").unwrap().edit = edit("task_title", "Replacement");
    assert!(saved.merge(&reused).is_err());
    let mut incomplete = saved.clone();
    incomplete
        .changes
        .get_mut("a")
        .unwrap()
        .parents
        .insert("missing".into());
    assert!(incomplete.to_bytes().is_err());
    let mut execution = serde_json::to_value(&saved).unwrap();
    execution["changes"]["a"]["edit"]["change"]["field"] = json!("worktree");
    assert!(PlanningSnapshot::from_bytes(&serde_json::to_vec(&execution).unwrap()).is_err());
    let empty = PlanningSnapshot {
        changes: BTreeMap::new(),
    };
    assert_eq!(saved.merge(&empty).unwrap(), saved);
}

#[test]
fn equal_ids_in_different_work_scopes_keep_independent_causes() {
    let mut snapshot = PlanningSnapshot::default();
    for (id, change) in [
        ("project", WorkValue::ProjectRank(1)),
        ("task", WorkValue::TaskRank(2)),
    ] {
        snapshot.changes.insert(
            id.into(),
            PlanningMutation {
                edit: WorkEdit {
                    target: "retained-id".into(),
                    change,
                },
                clock: 1,
                parents: BTreeSet::new(),
            },
        );
    }
    let retained = PlanningSnapshot::from_bytes(&snapshot.to_bytes().unwrap()).unwrap();
    assert_eq!(retained.winners().count(), 2);
    snapshot.changes.get_mut("task").unwrap().clock = 2;
    snapshot
        .changes
        .get_mut("task")
        .unwrap()
        .parents
        .insert("project".into());
    assert!(snapshot.validate().is_err());
}
