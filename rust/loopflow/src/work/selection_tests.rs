use super::selection::{
    move_into_place, Arrangement, ArrangementMode, DashboardId, MemberRef, Selection,
    SelectionError, SelectionExpression, SelectionId, SelectionResolution, SelectionState,
    SelectionWork, WorkWorkspaceKey, WorkWorkspaceKind,
};
use crate::id::WaveId;
use std::collections::{BTreeMap, BTreeSet};

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
#[test]
fn defaults_include_child_waves_and_working_wave_tasks() {
    let root = WaveId::new();
    let child = WaveId::new();
    let empty = WaveId::new();
    let work: Vec<_> = [
        (key(root.as_str(), WorkWorkspaceKind::Wave), None, None),
        (
            key(child.as_str(), WorkWorkspaceKind::Wave),
            Some(root.clone()),
            None,
        ),
        (
            key(empty.as_str(), WorkWorkspaceKind::Wave),
            Some(root.clone()),
            None,
        ),
        (
            key("unstarted", WorkWorkspaceKind::Task),
            Some(root.clone()),
            Some(true),
        ),
        (
            key("idle", WorkWorkspaceKind::Task),
            Some(child),
            Some(true),
        ),
        (
            key("backlog", WorkWorkspaceKind::Task),
            Some(root.clone()),
            Some(false),
        ),
        (
            key("done", WorkWorkspaceKind::Task),
            Some(root.clone()),
            Some(false),
        ),
        (
            key("unknown", WorkWorkspaceKind::Task),
            Some(root.clone()),
            None,
        ),
    ]
    .into_iter()
    .map(|(key, parent_wave, open)| SelectionWork {
        states: match key.id.as_str() {
            "backlog" => BTreeSet::from([SelectionState::Backlog]),
            "done" => BTreeSet::from([SelectionState::Done]),
            "unstarted" => BTreeSet::from([SelectionState::Unstarted]),
            "idle" => BTreeSet::from([SelectionState::Idle, SelectionState::Working]),
            _ => BTreeSet::new(),
        },
        key,
        parent_wave,
        open,
        name: String::new(),
        activity_known: false,
    })
    .collect();
    let wave = node("root", SelectionExpression::Wave { wave: root.clone() })
        .resolve("repo", &work, &BTreeMap::new())
        .unwrap();
    assert_eq!(
        wave.members
            .iter()
            .map(|member| member.work.id.as_str())
            .collect::<Vec<_>>(),
        [work[1].key.id.as_str(), empty.as_str(), "idle"]
    );
    assert_eq!(wave.unavailable.len(), 1);
    let filtered = Selection::parse(&format!("wave:{root} state:open"), None)
        .unwrap()
        .resolve("repo", &work, &BTreeMap::new())
        .unwrap();
    assert_eq!(
        filtered
            .members
            .iter()
            .map(|member| member.work.id.as_str())
            .collect::<Vec<_>>(),
        ["unstarted", "idle"]
    );
    assert_eq!(filtered.unavailable.len(), 1);
    let repo = node("root", SelectionExpression::Repository)
        .resolve("repo", &work, &BTreeMap::new())
        .unwrap();
    assert_eq!(repo.members.len(), 2); // immediate Wave plus the idle Task; unstarted isn't Working
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
        name: String::new(),
        states: BTreeSet::from([SelectionState::Idle, SelectionState::Working]),
        activity_known: false,
    }];
    let query = DashboardId("saved".into());
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
fn an_order_must_permute_every_occurrence_including_hidden_ones() {
    let a = member("a", "task-a");
    let b = member("b", "task-b");
    let c = member("c", "task-c");
    let mut arrangement = Arrangement::automatic(&[a.clone(), b.clone(), c.clone()]);
    arrangement.hide(&b, true).unwrap();
    for invalid in [
        vec![c.clone(), a.clone()],
        vec![c.clone(), a.clone(), a.clone()],
        vec![c.clone(), a.clone(), b.clone(), member("d", "task-d")],
    ] {
        let edit = Arrangement {
            order: invalid,
            ..arrangement.clone()
        };
        assert_eq!(
            edit.validate(&arrangement.order),
            Err(SelectionError::Membership)
        );
    }
    let edit = Arrangement {
        order: vec![c.clone(), a.clone(), b.clone()],
        ..arrangement.clone()
    };
    assert_eq!(edit.validate(&arrangement.order), Ok(()));
    assert_eq!(edit.project().visible, [c, a]);
}

#[test]
fn moving_into_place_is_a_move_not_a_swap() {
    let mut order = vec!["a", "b", "c"];
    move_into_place(&mut order, 2, 0);
    assert_eq!(order, ["c", "a", "b"]);
    move_into_place(&mut order, 1, 2);
    assert_eq!(order, ["c", "b", "a"]);
}

#[test]
fn hiding_in_the_middle_keeps_position_for_restore() {
    let [a, b, c] = ["a", "b", "c"].map(|id| member(id, id));
    let mut arrangement = Arrangement::automatic(&[a.clone(), b.clone(), c.clone()]);
    arrangement.hide(&b, true).unwrap();
    assert_eq!(arrangement.project().visible, [a.clone(), c.clone()]);
    arrangement.hide(&b, false).unwrap();
    assert_eq!(arrangement.project().visible, [a, b, c]);
}

#[test]
fn first_visible_occurrence_wins_and_hiding_it_promotes_the_duplicate() {
    let first = member("first", "task-a");
    let b = member("b", "task-b");
    let later = member("later", "task-a");
    let mut arrangement = Arrangement::automatic(&[first.clone(), b.clone(), later.clone()]);
    let projection = arrangement.project();
    assert_eq!(projection.visible, [first.clone(), b.clone()]);
    assert_eq!(projection.suppressed[0].winner, first);
    arrangement.hide(&first, true).unwrap();
    let projection = arrangement.project();
    assert_eq!(projection.visible, [b.clone(), later.clone()]);
    assert!(projection.suppressed.is_empty());
    arrangement.hide(&first, false).unwrap();
    arrangement.order = vec![later.clone(), b, first.clone()];
    assert_eq!(arrangement.project().suppressed[0].winner, later);
}

#[test]
fn incomplete_reads_keep_members_and_complete_departures_prune_the_order() {
    let [a, b, c] = ["a", "b", "c"].map(|id| member(id, id));
    let mut arrangement = Arrangement {
        mode: ArrangementMode::Touched,
        ..Arrangement::automatic(&[b.clone(), a.clone()])
    };
    arrangement.hide(&a, true).unwrap();
    arrangement.reconcile(&SelectionResolution {
        attention: Default::default(),
        members: vec![b.clone(), c.clone()],
        unavailable: vec!["offline".into()],
    });
    assert_eq!(arrangement.order, [b.clone(), a.clone(), c.clone()]);
    assert!(arrangement.hidden.contains(&a));
    arrangement.reconcile(&SelectionResolution {
        attention: Default::default(),
        members: vec![c.clone(), b.clone()],
        unavailable: vec![],
    });
    assert_eq!(arrangement.order, [b, c]);
    assert!(arrangement.hidden.is_empty());
}

#[test]
fn incomplete_auto_refresh_does_not_promote_a_duplicate_past_the_retained_winner() {
    let first = member("first", "task");
    let second = member("second", "task");
    let mut arrangement = Arrangement::automatic(&[first.clone(), second.clone()]);
    arrangement.reconcile(&SelectionResolution {
        attention: Default::default(),
        members: vec![second],
        unavailable: vec!["first source unavailable".into()],
    });
    assert_eq!(arrangement.project().visible, [first]);
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
    let query = DashboardId("invalid".into());
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

fn query_work() -> Vec<SelectionWork> {
    let wave = WaveId::new();
    vec![
        SelectionWork {
            key: key(wave.as_str(), WorkWorkspaceKind::Wave),
            parent_wave: None,
            open: None,
            name: "Product".into(),
            states: Default::default(),
            activity_known: false,
        },
        SelectionWork {
            key: key("a", WorkWorkspaceKind::Task),
            parent_wave: Some(wave.clone()),
            open: Some(true),
            name: "FIX-1".into(),
            states: [SelectionState::Open, SelectionState::Available].into(),
            activity_known: true,
        },
        SelectionWork {
            key: key("b", WorkWorkspaceKind::Task),
            parent_wave: Some(wave),
            open: Some(false),
            name: "FIX-2".into(),
            states: [SelectionState::Backlog].into(),
            activity_known: true,
        },
    ]
}

#[test]
fn query_selection_filters_compose_with_defaults_and_preserve_overlapping_occurrences() {
    let work = query_work();
    let query = Selection::parse(
        "wave:Product (state:open or state:backlog) or task:#1",
        None,
    )
    .unwrap();
    let reading = query.resolve("repo", &work, &BTreeMap::new()).unwrap();
    assert_eq!(
        reading
            .members
            .iter()
            .map(|member| member.work.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "a"]
    );
    assert!(reading.unavailable.is_empty());
    let layout = Arrangement::automatic(&reading.members);
    let projection = layout.project();
    assert_eq!(projection.suppressed.len(), 1);
    assert_eq!(projection.suppressed[0].winner, reading.members[0]);
    let named = DashboardId("attention".into());
    let catalog = BTreeMap::from([(named, Selection::parse("state:available", None).unwrap())]);
    let query = Selection::parse("query:attention or task:FIX-2", None).unwrap();
    assert_eq!(
        query
            .resolve("repo", &work, &catalog)
            .unwrap()
            .members
            .len(),
        2
    );
}

#[test]
fn query_selection_edits_retain_source_ids_and_hidden_geometry() {
    let work = query_work();
    let open = Selection::parse("state:open", None).unwrap();
    let edited = Selection::parse("state:available", Some(&open)).unwrap();
    assert_eq!(
        open.resolve("repo", &work, &BTreeMap::new())
            .unwrap()
            .members,
        edited
            .resolve("repo", &work, &BTreeMap::new())
            .unwrap()
            .members
    );
    let single = Selection::parse("task:FIX-1", None).unwrap();
    let single_reading = single.resolve("repo", &work, &BTreeMap::new()).unwrap();
    let expanded = Selection::parse("task:FIX-1 or task:FIX-2", Some(&single)).unwrap();
    assert_eq!(
        expanded
            .resolve("repo", &work, &BTreeMap::new())
            .unwrap()
            .members[0],
        single_reading.members[0]
    );
    let before = Selection::parse("task:FIX-1 or task:FIX-1", None).unwrap();
    assert_eq!(
        before,
        Selection::parse("task:FIX-1 or task:FIX-1", None).unwrap()
    );
    let old = before.resolve("repo", &work, &BTreeMap::new()).unwrap();
    let after = Selection::parse("task:FIX-2 or task:FIX-1 or task:FIX-1", Some(&before)).unwrap();
    let next = after.resolve("repo", &work, &BTreeMap::new()).unwrap();
    assert_eq!(old.members, next.members[1..]);
    let mut layout = Arrangement::automatic(&old.members);
    layout.mode = ArrangementMode::Touched;
    layout.hide(&old.members[0], true).unwrap();
    layout.reconcile(&next);
    assert_eq!(&layout.order[..2], old.members);
    assert!(layout.hidden.contains(&old.members[0]));
    assert_eq!(layout.project().visible[0], old.members[1]);
}

#[test]
fn query_selection_edits_keep_restored_and_fresh_ids_distinct() {
    // Earlier edits move source-derived IDs onto other clauses; returning to
    // the original text must not hand one clause's retained ID to a fresh one.
    let mut previous = None;
    for source in [
        "task:FIX-1 or task:FIX-3",
        "task:FIX-3 or task:FIX-2",
        "task:FIX-2 or task:FIX-1",
    ] {
        previous = Some(Selection::parse(source, previous.as_ref()).unwrap());
    }
    let before = previous.unwrap();
    let after = Selection::parse("task:FIX-1 or task:FIX-3", Some(&before)).unwrap();
    let clause = |selection: &Selection, index: usize| match &selection.expression {
        SelectionExpression::Compose { items } => items[index].id.clone(),
        other => panic!("expected composition, found {other:?}"),
    };
    assert_eq!(clause(&before, 1), clause(&after, 0));
    assert_ne!(clause(&after, 0), clause(&after, 1));
}

#[test]
fn query_selection_rejects_bad_syntax_and_ambiguous_refs_without_guessing() {
    for source in [
        "",
        "repo:other",
        "state:waiting",
        "kind:repo",
        "task:",
        "task:\"open",
        "(state:open",
        "state:open)",
        "state:open or",
        "state:open and or task:a",
        "unknown:a",
    ] {
        assert!(Selection::parse(source, None).is_err(), "{source}");
    }
    let mut work = query_work();
    work[2].name = "OTHER-1".into();
    assert!(Selection::parse("task:#1", None)
        .unwrap()
        .resolve("repo", &work, &BTreeMap::new())
        .is_err());
    let missing = Selection::parse("query:missing", None)
        .unwrap()
        .resolve("repo", &work, &BTreeMap::new())
        .unwrap();
    assert!(!missing.unavailable.is_empty());
    let empty = Selection::parse("task:FIX-1 state:backlog query:missing", None)
        .unwrap()
        .resolve("repo", &work, &BTreeMap::new())
        .unwrap();
    assert!(empty.members.is_empty());
    assert_eq!(empty.unavailable, missing.unavailable);
    work[1].activity_known = false;
    assert!(!Selection::parse("state:busy", None)
        .unwrap()
        .resolve("repo", &work, &BTreeMap::new())
        .unwrap()
        .unavailable
        .is_empty());
}

#[test]
fn query_selection_plan_wire_roundtrips() {
    let fixture = include_str!("../../../../tests/fixtures/dto/view_plan.json");
    let plan: super::selection::ViewPlan = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        serde_json::to_value(&plan).unwrap(),
        serde_json::from_str::<serde_json::Value>(fixture).unwrap()
    );
    assert_eq!(plan.projection, plan.arrangement.project());
    plan.arrangement.validate(&plan.resolution.members).unwrap();
}

#[test]
fn dashboard_wire_keeps_identity_separate_from_its_query() {
    let fixture = include_str!("../../../../tests/fixtures/dto/dashboard.json");
    let dashboard: super::selection::Dashboard = serde_json::from_str(fixture).unwrap();
    assert_eq!(dashboard.id.0, "picked");
    assert_ne!(dashboard.id.0, dashboard.selection.id.0);
    assert_eq!(
        serde_json::to_value(&dashboard).unwrap(),
        serde_json::from_str::<serde_json::Value>(fixture).unwrap()
    );
}

#[test]
fn auto_order_follows_the_query_until_touched_then_arrivals_append() {
    let work = query_work();
    let query = Selection::parse("kind:task", None).unwrap();
    let reading = query.resolve("repo", &work, &BTreeMap::new()).unwrap();
    let mut layout = Arrangement::automatic(&[]);
    layout.reconcile(&reading);
    assert_eq!(layout.order, reading.members);
    let mut reversed = layout.order.clone();
    reversed.reverse();
    layout.order = reversed.clone();
    layout.mode = ArrangementMode::Touched;
    let wider = Selection::parse("kind:task or kind:wave", Some(&query)).unwrap();
    let wider = wider.resolve("repo", &work, &BTreeMap::new()).unwrap();
    let arrivals: Vec<_> = wider
        .members
        .iter()
        .filter(|member| !reversed.contains(member))
        .cloned()
        .collect();
    assert!(!arrivals.is_empty());
    layout.reconcile(&wider);
    assert_eq!(&layout.order[..reversed.len()], reversed);
    assert_eq!(&layout.order[reversed.len()..], arrivals);
}

#[test]
fn selection_portable_panes_round_trip_all_content_and_recovery() {
    let fixture = include_str!("../../../../tests/fixtures/dto/pane_layout.json");
    let layout: super::selection::WorkPaneLayout = serde_json::from_str(fixture).unwrap();
    layout.validate().unwrap();
    assert_eq!(
        serde_json::to_value(&layout).unwrap(),
        serde_json::from_str::<serde_json::Value>(fixture).unwrap()
    );
    assert_eq!(layout.drafts[0].text, "unsaved edits");
    assert_ne!(layout.focused, layout.zoomed);
}

#[test]
fn working_tasks_are_the_working_section_and_the_others_have_their_own_queries() {
    let wave = WaveId::new();
    let work: Vec<_> = [
        ("idle", SelectionState::Idle, SelectionState::Working),
        ("active", SelectionState::Active, SelectionState::Working),
        ("moving", SelectionState::Moving, SelectionState::Working),
        ("unassigned", SelectionState::Idle, SelectionState::Backlog),
        (
            "unstarted",
            SelectionState::Unstarted,
            SelectionState::Backlog,
        ),
        (
            "shipping",
            SelectionState::Shipping,
            SelectionState::Autonomous,
        ),
    ]
    .into_iter()
    .map(|(id, state, section)| SelectionWork {
        key: key(id, WorkWorkspaceKind::Task),
        parent_wave: Some(wave.clone()),
        open: Some(true),
        name: id.into(),
        states: BTreeSet::from([SelectionState::Open, state, section]),
        activity_known: true,
    })
    .chain([SelectionWork {
        key: key(wave.as_str(), WorkWorkspaceKind::Wave),
        parent_wave: None,
        open: None,
        name: "wave".into(),
        states: BTreeSet::new(),
        activity_known: false,
    }])
    .collect();
    let ids = |selection: Selection| {
        selection
            .resolve("repo", &work, &BTreeMap::new())
            .unwrap()
            .task_ids()
    };
    assert_eq!(
        ids(node("repo", SelectionExpression::Repository)),
        ["idle", "active", "moving"]
    );
    assert_eq!(
        ids(node(
            "wave",
            SelectionExpression::Wave { wave: wave.clone() }
        )),
        ["idle", "active", "moving"]
    );
    assert_eq!(
        ids(Selection::parse("state:autonomous", None).unwrap()),
        ["shipping"]
    );
    assert_eq!(
        ids(Selection::parse("state:backlog", None).unwrap()),
        ["unassigned", "unstarted"]
    );
    assert_eq!(
        ids(Selection::parse("state:moving", None).unwrap()),
        ["moving"]
    );
}
