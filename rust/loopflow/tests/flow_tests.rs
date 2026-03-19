use std::fs;
use std::path::Path;

use loopflow::engine::flow::{ConcreteItem, FlowItem, Step};
use loopflow::engine::{expand_flow, load_flow};
use tempfile::TempDir;

fn write_step(repo: &Path, name: &str, content: &str) {
    let steps_dir = repo.join(".lf/steps");
    fs::create_dir_all(&steps_dir).unwrap();
    fs::write(steps_dir.join(format!("{name}.md")), content).unwrap();
}

fn write_flow(repo: &Path, name: &str, content: &str) {
    let flows_dir = repo.join(".lf/flows");
    fs::create_dir_all(&flows_dir).unwrap();
    fs::write(flows_dir.join(format!("{name}.yaml")), content).unwrap();
}

fn expand_named_flow(repo: &Path, name: &str) -> Vec<ConcreteItem> {
    let flow = load_flow(name, repo).unwrap();
    expand_flow(&flow, repo).unwrap()
}

fn assert_step_name(item: &ConcreteItem, expected: &str) {
    match item {
        ConcreteItem::Step(step) => assert_eq!(step.step.name, expected),
        other => panic!("expected step {expected}, got {other:?}"),
    }
}

fn assert_step_sequence(items: &[ConcreteItem], expected: &[&str]) {
    assert_eq!(items.len(), expected.len());
    for (item, step_name) in items.iter().zip(expected) {
        assert_step_name(item, step_name);
    }
}

#[test]
fn flow_parsing_parity() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "sample",
        r#"
- implement
- step:
    name: review
    interactive: true
    direction: [ux, security]
"#,
    );

    let flow = load_flow("sample", repo).unwrap();
    assert_eq!(flow.name, "sample");
    assert_eq!(flow.items.len(), 2);
    assert_eq!(
        flow.items[0],
        FlowItem::Step(Step {
            name: "implement".to_string(),
            agent: None,
            default_agent: None,
            directions: vec![],
            action_style: None,
            interactive: None,
            content: None,
            fast_path: None,
        })
    );
    assert_eq!(
        flow.items[1],
        FlowItem::Step(Step {
            name: "review".to_string(),
            agent: None,
            default_agent: None,
            directions: vec!["ux".to_string(), "security".to_string()],
            action_style: None,
            interactive: Some(true),
            content: None,
            fast_path: None,
        })
    );
}

#[test]
fn golden_flows() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "forked",
        r#"
- and:
    branches:
      - step: { name: implement }
      - step: { name: polish }
- and:
    branches:
      - step: { name: quick }
      - step: { name: deep }
- flow: nested
"#,
    );

    let flow = load_flow("forked", repo).unwrap();
    assert_eq!(flow.items.len(), 3);
    match &flow.items[0] {
        FlowItem::And { branches } => {
            assert_eq!(branches.len(), 2);
        }
        _ => panic!("expected and"),
    }
    match &flow.items[1] {
        FlowItem::And { branches } => {
            assert_eq!(branches.len(), 2);
        }
        _ => panic!("expected and"),
    }
    match &flow.items[2] {
        FlowItem::FlowRef(name) => {
            assert_eq!(name, "nested");
        }
        _ => panic!("expected flow ref"),
    }
}

#[test]
fn and_select_is_rejected() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "forked",
        r#"
- and:
    branches:
      - step: { name: implement }
    select: all
"#,
    );

    let err = load_flow("forked", repo).expect_err("and select should fail");
    let message = err.to_string();
    assert!(message.contains("and select modes are not supported"));
}

#[test]
fn flow_ref_parses_into_items() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "child",
        r#"
- implement
"#,
    );
    write_flow(
        repo,
        "parent",
        r#"
- flow: child
- review
"#,
    );

    let flow = load_flow("parent", repo).unwrap();
    assert_eq!(flow.items.len(), 2);
    assert!(matches!(flow.items[0], FlowItem::FlowRef(_)));
    assert!(matches!(flow.items[1], FlowItem::Step(_)));
}

#[test]
fn ops_item_parses_and_expands() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "ship-ish",
        r#"
- implement
- ops: land --create-pr
"#,
    );

    let flow = load_flow("ship-ish", repo).unwrap();
    assert_eq!(flow.items.len(), 2);
    match &flow.items[1] {
        FlowItem::Op(item) => {
            assert_eq!(item.command, "land");
            assert_eq!(item.args, vec!["--create-pr"]);
        }
        other => panic!("expected ops item, got {other:?}"),
    }

    let expanded = expand_flow(&flow, repo).unwrap();
    assert!(matches!(&expanded[1], ConcreteItem::Op(_)));
}

#[test]
fn expand_flow_tracks_parents() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "child",
        r#"
- implement
"#,
    );
    write_flow(
        repo,
        "parent",
        r#"
- flow: child
- review
"#,
    );

    let flow = load_flow("parent", repo).unwrap();
    let items = expand_flow(&flow, repo).unwrap();
    match &items[0] {
        ConcreteItem::Step(step) => {
            assert_eq!(step.step.name, "implement");
            assert_eq!(step.flow_parents, vec!["parent", "child"]);
        }
        _ => panic!("expected expanded step"),
    }
}

/// Plain string items in flow YAML that match a sub-flow name should be
/// expanded as sub-flows, not treated as step names.
#[test]
fn expand_flow_resolves_plain_string_as_subflow() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    write_flow(repo, "publish", "- step-a\n- step-b");
    write_flow(repo, "parent", "- review\n- publish");

    let items = expand_named_flow(repo, "parent");

    assert_eq!(items.len(), 3, "publish should expand into its sub-steps");
    assert_step_name(&items[0], "review");
    match &items[1] {
        ConcreteItem::Step(s) => {
            assert_eq!(s.step.name, "step-a");
            assert_eq!(s.flow_parents, vec!["parent", "publish"]);
        }
        _ => panic!("expected step from publish sub-flow"),
    }
    match &items[2] {
        ConcreteItem::Step(s) => {
            assert_eq!(s.step.name, "step-b");
            assert_eq!(s.flow_parents, vec!["parent", "publish"]);
        }
        _ => panic!("expected step from publish sub-flow"),
    }
}

/// A plain string that is both a step name AND a flow name should NOT
/// be expanded as a sub-flow (step takes priority to avoid ambiguity).
#[test]
fn expand_flow_prefers_step_over_single_step_flow() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    write_step(repo, "review", "Review the code.");
    write_flow(repo, "parent", "- review");

    let items = expand_named_flow(repo, "parent");

    assert_eq!(items.len(), 1);
    match &items[0] {
        ConcreteItem::Step(s) => {
            assert_eq!(s.step.name, "review");
            assert_eq!(s.flow_parents, vec!["parent"]);
        }
        _ => panic!("expected step"),
    }
}

/// The builtin wave-reduce flow should expand to 2 items:
/// and(reduce×3), update-wave.
#[test]
fn builtin_wave_reduce_expands_to_update_wave() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "wave-reduce");

    // and + update-wave = 2
    assert_eq!(
        items.len(),
        2,
        "wave-reduce should expand into and + update-wave"
    );

    // Step 0: and (reduce × 3 directions)
    assert!(
        matches!(&items[0], ConcreteItem::And(_)),
        "expected and at index 0"
    );
    if let ConcreteItem::And(and) = &items[0] {
        let branch_directions: Vec<Vec<String>> = and
            .branches
            .iter()
            .map(|branch| branch.directions.clone())
            .collect();
        assert_eq!(
            branch_directions,
            vec![
                vec!["infra".to_string()],
                vec!["ux".to_string()],
                vec!["ceo".to_string()]
            ]
        );
    }
    // Step 1: update-wave
    assert_step_name(&items[1], "update-wave");
}

#[test]
fn builtin_ship_uses_ops_land_item() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "ship");
    assert!(!items.is_empty());
    assert!(matches!(items.last(), Some(ConcreteItem::Op(_))));
}

#[test]
fn builtin_tend_flow_structure() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "tend");

    // tend: scan-waves, or(router: assess)
    assert_eq!(items.len(), 2);
    assert_step_name(&items[0], "tend/scan-waves");
    match &items[1] {
        ConcreteItem::Or(or_def) => {
            assert_eq!(
                or_def.router.as_deref(),
                Some("tend/assess"),
                "or should have tend/assess as router"
            );
            assert_eq!(or_def.paths.len(), 2);
            assert!(or_def.paths.contains_key("tune"));
            assert!(or_def.paths.contains_key("silence"));
            assert_eq!(or_def.paths["tune"].flow.as_deref(), Some("tend-tune"));
            assert_eq!(or_def.paths["silence"].flow, None);
            assert_eq!(or_def.paths["silence"].step, None);
        }
        other => panic!("expected Or, got {other:?}"),
    }

    let tune_items = expand_named_flow(repo, "tend-tune");
    assert_eq!(tune_items.len(), 3);
    assert_step_name(&tune_items[0], "tend/draft-chord");
    assert_step_name(&tune_items[1], "tend/review-chord");
    assert_step_name(&tune_items[2], "tend/apply-chord");
}

#[test]
fn builtin_vsm_flow_structure() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "vsm");

    assert_step_sequence(&items, &["vsm/s5", "vsm/s4", "vsm/s3", "vsm/s2", "vsm/s1"]);
}

#[test]
fn builtin_ship_roadmap_has_ops_in_or_subflow() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "ship-roadmap");
    let play_flow_name = match &items[1] {
        ConcreteItem::Or(or_def) => or_def.paths["play"]
            .flow
            .as_deref()
            .expect("play path should point at a sub-flow"),
        other => panic!("expected or in ship-roadmap, got {other:?}"),
    };
    let items = expand_named_flow(repo, play_flow_name);
    assert!(
        items.iter().any(|item| matches!(item, ConcreteItem::Op(_))),
        "ship-roadmap-play should contain an ops item"
    );
}
