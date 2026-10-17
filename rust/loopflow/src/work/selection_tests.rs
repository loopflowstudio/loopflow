use super::selection::{
    Arrangement, ArrangementAxis, ArrangementTree, MemberRef, QueryId, Selection, SelectionError,
    SelectionExpression, SelectionId, SelectionResolution, SelectionWork, WorkWorkspaceKey,
    WorkWorkspaceKind,
};
use crate::id::WaveId;
use std::collections::BTreeMap;

fn key(id: &str, kind: WorkWorkspaceKind) -> WorkWorkspaceKey {
    WorkWorkspaceKey {
        repository: "repo".into(),
        kind,
        id: id.into(),
    }
}
fn node(id: &str, expression: SelectionExpression) -> Selection {
    Selection {
        id: SelectionId(id.into()),
        expression,
    }
}
fn member(source: &str, id: &str) -> MemberRef {
    MemberRef {
        source: vec![SelectionId(source.into())],
        work: key(id, WorkWorkspaceKind::Task),
    }
}
fn split(first: ArrangementTree, second: ArrangementTree, ratio: f64) -> ArrangementTree {
    ArrangementTree::Split {
        axis: ArrangementAxis::Vertical,
        ratio,
        first: Box::new(first),
        second: Box::new(second),
    }
}
fn leaf(member: &MemberRef) -> ArrangementTree {
    ArrangementTree::Member {
        member: member.clone(),
    }
}

#[test]
fn defaults_include_child_waves_and_descendant_tasks_without_execution_filters() {
    let root = WaveId::new();
    let child = WaveId::new();
    let empty = WaveId::new();
    let work = vec![
        SelectionWork {
            key: key(root.as_str(), WorkWorkspaceKind::Wave),
            parent_wave: None,
            open: None,
        },
        SelectionWork {
            key: key(child.as_str(), WorkWorkspaceKind::Wave),
            parent_wave: Some(root.clone()),
            open: None,
        },
        SelectionWork {
            key: key(empty.as_str(), WorkWorkspaceKind::Wave),
            parent_wave: Some(root.clone()),
            open: None,
        },
        SelectionWork {
            key: key("paused", WorkWorkspaceKind::Task),
            parent_wave: Some(root.clone()),
            open: Some(true),
        },
        SelectionWork {
            key: key("idle", WorkWorkspaceKind::Task),
            parent_wave: Some(child),
            open: Some(true),
        },
        SelectionWork {
            key: key("backlog", WorkWorkspaceKind::Task),
            parent_wave: Some(root.clone()),
            open: Some(false),
        },
        SelectionWork {
            key: key("done", WorkWorkspaceKind::Task),
            parent_wave: Some(root.clone()),
            open: Some(false),
        },
        SelectionWork {
            key: key("unknown", WorkWorkspaceKind::Task),
            parent_wave: Some(root.clone()),
            open: None,
        },
    ];
    let wave = node("root", SelectionExpression::Wave { wave: root.clone() })
        .resolve("repo", &work, &BTreeMap::new())
        .unwrap();
    assert_eq!(
        wave.members
            .iter()
            .map(|member| member.work.id.as_str())
            .collect::<Vec<_>>(),
        [work[1].key.id.as_str(), empty.as_str(), "paused", "idle"]
    );
    assert_eq!(wave.unavailable.len(), 1);
    let repo = node("root", SelectionExpression::Repository)
        .resolve("repo", &work, &BTreeMap::new())
        .unwrap();
    assert_eq!(repo.members.len(), 3); // immediate Wave plus both recursive Tasks
    let explicit = node(
        "backlog",
        SelectionExpression::Work {
            work: work[5].key.clone(),
        },
    )
    .resolve("repo", &work, &BTreeMap::new())
    .unwrap();
    assert_eq!(explicit.members[0].work.id, "backlog");
}

#[test]
fn nested_sources_survive_insertions_and_report_unavailable_queries_and_cycles() {
    let work = vec![SelectionWork {
        key: key("task", WorkWorkspaceKind::Task),
        parent_wave: None,
        open: Some(true),
    }];
    let query = QueryId("saved".into());
    let reference = node(
        "reference",
        SelectionExpression::Query {
            query: query.clone(),
        },
    );
    let mut queries = BTreeMap::from([(
        query.clone(),
        node("definition", SelectionExpression::Repository),
    )]);
    let before = reference.resolve("repo", &work, &queries).unwrap();
    let mut composition = node(
        "composition",
        SelectionExpression::Compose {
            items: vec![reference.clone()],
        },
    );
    let first = composition.resolve("repo", &work, &queries).unwrap();
    if let SelectionExpression::Compose { items } = &mut composition.expression {
        items.insert(0, node("inserted", SelectionExpression::Repository));
    }
    let after = composition.resolve("repo", &work, &queries).unwrap();
    assert_eq!(first.members[0], after.members[1]);
    assert_ne!(after.members[0], after.members[1]);
    assert_eq!(
        before.members[0].source,
        [
            SelectionId("reference".into()),
            SelectionId("definition".into())
        ]
    );
    queries.insert(query.clone(), reference.clone());
    assert!(matches!(
        reference.resolve("repo", &work, &queries),
        Err(SelectionError::QueryCycle(_))
    ));
    let missing = reference.resolve("repo", &work, &BTreeMap::new()).unwrap();
    assert!(missing.members.is_empty());
    assert!(!missing.unavailable.is_empty());
}

#[test]
fn hidden_members_survive_complete_edits_and_first_wins_promotes_in_tree_order() {
    let a = member("first", "task-a");
    let b = member("b", "task-b");
    let duplicate = member("later", "task-a");
    let mut arrangement = Arrangement::automatic(&[a.clone(), b.clone(), duplicate.clone()]);
    assert_eq!(arrangement.project().suppressed[0].winner, a);
    arrangement.hide(&a, true).unwrap();
    assert!(arrangement.project().suppressed.is_empty());
    let before = arrangement.clone();
    assert_eq!(
        arrangement.replace(Some(leaf(&b))),
        Err(SelectionError::Membership)
    );
    assert_eq!(arrangement, before);
    assert_eq!(
        arrangement.replace(Some(split(
            leaf(&a),
            split(leaf(&b), leaf(&duplicate), 0.7),
            f64::NAN
        ))),
        Err(SelectionError::Ratio)
    );
    assert_eq!(arrangement, before);
    let edited = split(leaf(&duplicate), split(leaf(&b), leaf(&a), 0.7), 0.65);
    arrangement.replace(Some(edited.clone())).unwrap();
    arrangement.hide(&a, false).unwrap();
    assert_eq!(arrangement.tree, Some(edited));
    assert_eq!(arrangement.project().suppressed[0].winner, duplicate);
    arrangement.hide(&duplicate, true).unwrap();
    assert!(arrangement
        .project()
        .visible
        .unwrap()
        .members()
        .contains(&a));
}

#[test]
fn incomplete_reads_keep_members_and_complete_departures_prune_current_geometry() {
    let a = member("a", "a");
    let b = member("b", "b");
    let c = member("c", "c");
    let mut arrangement = Arrangement::automatic(&[a.clone(), b.clone()]);
    arrangement
        .replace(Some(split(leaf(&a), leaf(&b), 0.7)))
        .unwrap();
    arrangement.hide(&a, true).unwrap();
    arrangement.reconcile(&SelectionResolution {
        members: vec![b.clone(), c.clone()],
        unavailable: vec!["offline".into()],
    });
    assert_eq!(arrangement.members(), [a.clone(), b.clone(), c.clone()]);
    assert!(arrangement.hidden.contains(&a));
    arrangement.reconcile(&SelectionResolution {
        members: vec![b.clone(), c.clone()],
        unavailable: vec![],
    });
    assert_eq!(arrangement.members(), [b, c]);
    assert!(arrangement.hidden.is_empty());
}

#[test]
fn incomplete_auto_refresh_does_not_promote_a_duplicate_past_the_retained_winner() {
    let first = member("first", "task");
    let second = member("second", "task");
    let mut arrangement = Arrangement::automatic(&[first.clone(), second.clone()]);
    arrangement.reconcile(&SelectionResolution {
        members: vec![second],
        unavailable: vec!["first source unavailable".into()],
    });
    assert_eq!(arrangement.project().visible.unwrap().members(), [first]);
}

#[test]
fn invalid_authored_ids_fail_in_compositions_and_referenced_definitions() {
    let duplicate = node("same", SelectionExpression::Repository);
    let invalid = node(
        "root",
        SelectionExpression::Compose {
            items: vec![duplicate.clone(), duplicate],
        },
    );
    assert_eq!(
        invalid.resolve("repo", &[], &BTreeMap::new()),
        Err(SelectionError::NodeIdentity("same".into()))
    );
    let query = QueryId("invalid".into());
    let reference = node(
        "reference",
        SelectionExpression::Query {
            query: query.clone(),
        },
    );
    assert_eq!(
        reference.resolve("repo", &[], &BTreeMap::from([(query, invalid)])),
        Err(SelectionError::NodeIdentity("same".into()))
    );
    assert_eq!(
        node("", SelectionExpression::Repository).resolve("repo", &[], &BTreeMap::new()),
        Err(SelectionError::NodeIdentity(String::new()))
    );
}
