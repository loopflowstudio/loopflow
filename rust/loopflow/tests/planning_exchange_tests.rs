use std::collections::{BTreeMap, BTreeSet};

use loopflow::engine::planning_exchange::{
    LinearObservation, PlanningKind, PlanningMutation, PlanningObject, PlanningSnapshot,
};
use serde_json::json;

fn write(
    snapshot: &mut PlanningSnapshot,
    id: &str,
    field: &str,
    value: &str,
    clock: i64,
    linear: bool,
) {
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
            linear: linear.then(|| {
                let mut item: loopflow::pm::PmItem = serde_json::from_value(
                    serde_json::from_str::<serde_json::Value>(include_str!(
                        "../../../tests/fixtures/dto/task_history_planning.json"
                    ))
                    .unwrap()["items"][0]
                        .clone(),
                )
                .unwrap();
                item.name = value.into();
                item.state = Some(value.into());
                item.revision = Some("2026-10-08T10:00:00Z".into());
                LinearObservation {
                    body: serde_json::to_value(item).unwrap(),
                    observed_at: 1,
                }
            }),
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
    write(&mut base, "initial", "issue_title", "Initial", 1, false);
    let mut laptop = base.clone();
    let mut worker = base;
    write(&mut laptop, "left", "issue_title", "Laptop", 2, false);
    write(&mut worker, "right", "issue_title", "Worker", 3, false);
    write(
        &mut worker,
        "brief",
        "issue_description",
        "Independent",
        4,
        false,
    );
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
fn observed_reopening_beats_delayed_completion_and_linear_beats_concurrent_peer() {
    let mut completed = PlanningSnapshot::default();
    write(
        &mut completed,
        "complete",
        "disposition",
        "completed",
        1,
        true,
    );
    let mut reopened = completed.clone();
    write(&mut reopened, "reopen", "disposition", "open", 2, false);
    assert_eq!(
        value(&completed.merge(&reopened).unwrap(), "disposition"),
        "open"
    );
    let mut observed = completed.clone();
    write(
        &mut observed,
        "linear-reopen",
        "disposition",
        "Linear open",
        3,
        true,
    );
    write(
        &mut reopened,
        "later",
        "disposition",
        "Peer completed",
        100,
        false,
    );
    let merged = reopened.merge(&observed).unwrap();
    assert_eq!(value(&merged, "disposition"), "Linear open");
    let (id, winner) = merged.winners().next().unwrap();
    assert_eq!(id, "linear-reopen");
    assert!(winner.linear.is_some());
    assert_eq!(winner, &merged.changes[id]);
    assert_eq!(
        merged.changes["later"].value["planning_state"],
        "Peer completed"
    );
}

#[test]
fn equal_values_retain_both_causes_and_ties_use_stable_identity() {
    let mut left = PlanningSnapshot::default();
    let mut right = PlanningSnapshot::default();
    write(&mut left, "a", "issue_title", "Same", 1, false);
    write(&mut right, "b", "issue_title", "Same", 1, false);
    let mut merged = left.merge(&right).unwrap();
    write(&mut merged, "c", "issue_title", "Next", 2, false);
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
    write(&mut saved, "a", "issue_title", "Saved", 1, false);
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

#[test]
fn concurrent_linear_facts_follow_provider_revision_not_the_receivers_clock() {
    let mut newer = PlanningSnapshot::default();
    write(&mut newer, "newer", "issue_title", "Newer", 2, true);
    newer
        .changes
        .get_mut("newer")
        .unwrap()
        .linear
        .as_mut()
        .unwrap()
        .body["revision"] = json!("2026-10-08T12:00:00Z");
    let mut delayed = PlanningSnapshot::default();
    write(&mut delayed, "delayed", "issue_title", "Older", 100, true);
    let merged = newer.merge(&delayed).unwrap();
    assert_eq!(value(&merged, "issue_title"), "Newer");
    assert_eq!(merged.changes["delayed"].value, "Older");
}

#[test]
fn provider_observations_cannot_smuggle_execution_or_invalid_revisions() {
    let mut snapshot = PlanningSnapshot::default();
    write(
        &mut snapshot,
        "observed",
        "issue_title",
        "Observed",
        1,
        true,
    );
    snapshot
        .changes
        .get_mut("observed")
        .unwrap()
        .linear
        .as_mut()
        .unwrap()
        .body["worktree"] = json!("/private/path");
    assert!(snapshot.to_bytes().is_err());
    snapshot
        .changes
        .get_mut("observed")
        .unwrap()
        .linear
        .as_mut()
        .unwrap()
        .body
        .as_object_mut()
        .unwrap()
        .remove("worktree");
    snapshot
        .changes
        .get_mut("observed")
        .unwrap()
        .linear
        .as_mut()
        .unwrap()
        .body["revision"] = json!("not-a-revision");
    assert!(snapshot.to_bytes().is_err());
}

#[test]
fn cross_origin_observation_retains_portable_causality_without_redirecting_identity() {
    let mut snapshot = PlanningSnapshot::default();
    write(&mut snapshot, "peer", "issue_title", "Observed", 1, true);
    snapshot.changes.get_mut("peer").unwrap().object.id = "task-peer".into();
    let mut observation = snapshot.changes["peer"].clone();
    observation.object.id = "task-existing".into();
    observation.clock = 2;
    observation.linear.as_mut().unwrap().observed_at = 10;
    observation.parents.insert("peer".into());
    snapshot.changes.insert("observed-here".into(), observation);
    write(
        &mut snapshot,
        "save",
        "issue_title",
        "Next local save",
        3,
        false,
    );
    let exchanged = PlanningSnapshot::from_bytes(&snapshot.to_bytes().unwrap()).unwrap();
    assert_eq!(
        exchanged.changes["save"].parents,
        BTreeSet::from(["observed-here".into()])
    );
    assert_eq!(
        exchanged.changes["observed-here"].parents,
        BTreeSet::from(["peer".into()])
    );
    assert_eq!(
        exchanged.heads().map(|(id, _)| id).collect::<Vec<_>>(),
        ["peer", "save"]
    );
    assert_eq!(snapshot.merge(&exchanged).unwrap(), snapshot);

    // Neither equal values nor another provider/revision can grant cross-origin
    // causality. The exact accepted fact is the proof, not the local association.
    for field in ["id", "revision"] {
        let mut unrelated = snapshot.clone();
        unrelated
            .changes
            .get_mut("observed-here")
            .unwrap()
            .linear
            .as_mut()
            .unwrap()
            .body[field] = json!(if field == "revision" {
            "2026-10-08T11:00:00Z"
        } else {
            "another-provider"
        });
        assert!(unrelated.to_bytes().is_err());
    }
    let mut unobserved = snapshot.clone();
    unobserved.changes.get_mut("observed-here").unwrap().linear = None;
    assert!(unobserved.to_bytes().is_err());
    let mut missing = snapshot;
    missing.changes.remove("peer");
    assert!(missing.to_bytes().is_err());
}

#[test]
fn peer_authored_predecessors_require_one_matching_provider_mapping() {
    let mut snapshot = PlanningSnapshot::default();
    write(
        &mut snapshot,
        "peer-mapping",
        "external_issue_id",
        "provider",
        1,
        false,
    );
    snapshot.changes.get_mut("peer-mapping").unwrap().object.id = "task-peer".into();
    write(
        &mut snapshot,
        "local-mapping",
        "external_issue_id",
        "provider",
        2,
        false,
    );
    write(
        &mut snapshot,
        "peer-edit",
        "issue_title",
        "Peer input",
        3,
        false,
    );
    snapshot.changes.get_mut("peer-edit").unwrap().object.id = "task-peer".into();
    write(
        &mut snapshot,
        "local-edit",
        "issue_title",
        "Continued locally",
        4,
        false,
    );
    snapshot
        .changes
        .get_mut("local-edit")
        .unwrap()
        .parents
        .insert("peer-edit".into());
    let decoded = PlanningSnapshot::from_bytes(&snapshot.to_bytes().unwrap()).unwrap();
    assert_eq!(decoded, snapshot);
    assert!(decoded.heads().any(|(id, _)| id == "peer-edit"));

    let mut contradictory = snapshot.clone();
    write(
        &mut contradictory,
        "another-mapping",
        "external_issue_id",
        "different-provider",
        5,
        false,
    );
    assert!(contradictory.to_bytes().is_err());
    let mut unknown = snapshot;
    unknown.changes.remove("peer-mapping");
    assert!(unknown.to_bytes().is_err());
}

#[test]
fn delegation_is_intent_not_connection_or_execution_authority() {
    let mut snapshot = PlanningSnapshot::default();
    let machine = loopflow::durable::MachineId::new();
    let assignment = json!({"machine_id":machine,"placed_at":1,"provenance":"explicit"});
    write(&mut snapshot, "assignment", "delegation", "", 1, false);
    snapshot.changes.get_mut("assignment").unwrap().value = assignment.clone();
    assert!(snapshot.to_bytes().is_ok());
    for invalid in [
        json!({"machine_id":machine,"placed_at":1,"provenance":"local_default"}),
        json!({"machine_id":"local","placed_at":1,"provenance":"explicit"}),
        json!({"machine_id":machine,"placed_at":-1,"provenance":"legacy"}),
        json!({"machine_id":machine,"placed_at":1,"provenance":"explicit","route":"ssh worker"}),
        json!({"machine_id":machine,"placed_at":1,"provenance":"explicit","checkout":"/private/work"}),
    ] {
        snapshot.changes.get_mut("assignment").unwrap().value = invalid;
        assert!(snapshot.to_bytes().is_err());
    }
    snapshot.changes.get_mut("assignment").unwrap().value = serde_json::Value::Null;
    assert!(snapshot.to_bytes().is_ok(), "null restores inheritance");
}
