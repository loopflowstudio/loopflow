use std::collections::{BTreeMap, BTreeSet};

use loopflow::engine::planning_exchange::{
    PlanningKind, PlanningMutation, PlanningObject, PlanningSnapshot,
};
use serde_json::json;

fn write(snapshot: &mut PlanningSnapshot, id: &str, field: &str, value: &str, clock: i64) {
    let object = PlanningObject {
        kind: PlanningKind::Task,
        id: "task-existing".into(),
    };
    let parents = snapshot
        .heads()
        .filter(|(_, change)| change.object == object && change.field == field)
        .map(|(id, _)| id.to_owned())
        .collect();
    snapshot.changes.insert(
        id.into(),
        PlanningMutation {
            object,
            field: field.into(),
            value: if field == "disposition" {
                json!({"planning_state":value,"planning_completed":0,"planning_completed_at":null})
            } else {
                json!(value)
            },
            clock,
            parents,
        },
    );
}

fn value(snapshot: &PlanningSnapshot, field: &str) -> serde_json::Value {
    let value = &snapshot
        .winners()
        .find(|(_, change)| change.field == field)
        .unwrap()
        .1
        .value;
    if field == "disposition" {
        value["planning_state"].clone()
    } else {
        value.clone()
    }
}

#[test]
fn concurrent_edits_converge_without_losing_independent_fields_or_losing_values() {
    let mut base = PlanningSnapshot::default();
    write(&mut base, "initial", "issue_title", "Initial", 1);
    let mut laptop = base.clone();
    let mut worker = base;
    write(&mut laptop, "left", "issue_title", "Laptop", 2);
    write(&mut worker, "right", "issue_title", "Worker", 3);
    write(&mut worker, "brief", "issue_description", "Independent", 4);
    let merged = laptop.merge(&worker).unwrap();
    assert_eq!(merged, worker.merge(&laptop).unwrap());
    assert_eq!(merged.merge(&laptop).unwrap(), merged);
    assert_eq!(value(&merged, "issue_title"), "Worker");
    assert_eq!(value(&merged, "issue_description"), "Independent");
    assert_eq!(merged.changes["left"].value, "Laptop");
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
    write(&mut left, "a", "issue_title", "Same", 1);
    write(&mut right, "b", "issue_title", "Same", 1);
    let mut merged = left.merge(&right).unwrap();
    write(&mut merged, "c", "issue_title", "Next", 2);
    assert_eq!(
        merged.changes["c"].parents,
        BTreeSet::from(["a".into(), "b".into()])
    );
    assert_eq!(value(&merged.merge(&left).unwrap(), "issue_title"), "Next");
    right.changes.get_mut("b").unwrap().value = json!("Tie winner");
    assert_eq!(
        value(&left.merge(&right).unwrap(), "issue_title"),
        "Tie winner"
    );
}

#[test]
fn conflicting_identity_missing_ancestors_and_execution_fields_are_rejected() {
    let mut saved = PlanningSnapshot::default();
    write(&mut saved, "a", "issue_title", "Saved", 1);
    let mut reused = saved.clone();
    reused.changes.get_mut("a").unwrap().value = json!("Replacement");
    assert!(saved.merge(&reused).is_err());
    let mut incomplete = saved.clone();
    incomplete
        .changes
        .get_mut("a")
        .unwrap()
        .parents
        .insert("missing".into());
    assert!(incomplete.to_bytes().is_err());
    let mut execution = saved.clone();
    execution.changes.get_mut("a").unwrap().field = "worktree".into();
    assert!(execution.to_bytes().is_err());
    let empty = PlanningSnapshot {
        changes: BTreeMap::new(),
    };
    assert_eq!(saved.merge(&empty).unwrap(), saved);
}
