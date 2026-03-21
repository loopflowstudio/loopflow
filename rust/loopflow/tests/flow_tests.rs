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
- op: land --create-pr
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

/// The builtin wave-reduce flow should expand to 3 items:
/// and(reduce×3), update-wave, deploy (which itself expands).
#[test]
fn builtin_wave_reduce_expands_to_update_wave() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "wave-reduce");

    // and + update-wave + deploy (gate + op:land + op:pm push-diff) = 5
    assert!(
        items.len() >= 3,
        "wave-reduce should expand into at least and + update-wave + deploy items, got {}",
        items.len()
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
fn builtin_deploy_uses_ops_land_item() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "deploy");
    assert!(!items.is_empty());
    assert!(matches!(&items[1], ConcreteItem::Op(_)));
}

#[test]
fn builtin_garden_or_silent_flow_structure() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "garden-or-silent");

    // garden-or-silent: op:pm pull, garden/scan, garden/assess, xor(garden, silence)
    assert_eq!(items.len(), 4);
    assert!(
        matches!(&items[0], ConcreteItem::Op(_)),
        "expected op:pm pull at index 0"
    );
    assert_step_name(&items[1], "garden/scan");
    assert_step_name(&items[2], "garden/assess");
    match &items[3] {
        ConcreteItem::Xor(xor_def) => {
            assert_eq!(xor_def.paths.len(), 2);
            assert!(xor_def.paths.contains_key("garden"));
            assert!(xor_def.paths.contains_key("silence"));
        }
        other => panic!("expected Xor, got {other:?}"),
    }
}

#[test]
fn builtin_governance_flows_structure() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let cases = [
        (
            "govern-identity",
            ["vsm/s5-scan", "vsm/s5-assess", "wave/mutate"],
        ),
        (
            "govern-intelligence",
            ["vsm/s4-scan", "vsm/s4-assess", "wave/mutate"],
        ),
        (
            "govern-control",
            ["vsm/s3-scan", "vsm/s3-assess", "wave/mutate"],
        ),
        (
            "govern-coordination",
            ["vsm/s2-scan", "vsm/s2-assess", "wave/mutate"],
        ),
    ];

    for (flow_name, expected) in cases {
        let items = expand_named_flow(repo, flow_name);
        assert_step_sequence(&items, &expected);
    }
}

#[test]
fn builtin_build_or_silent_has_xor_branch() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "build-or-silent");
    // op:pm pull, ingest, xor(build, silence)
    assert_eq!(items.len(), 3);
    assert!(
        matches!(&items[0], ConcreteItem::Op(_)),
        "expected op:pm pull at index 0"
    );
    assert_step_name(&items[1], "ingest");
    match &items[2] {
        ConcreteItem::Xor(xor_def) => {
            assert!(xor_def.paths.contains_key("build"));
            assert!(xor_def.paths.contains_key("silence"));
        }
        other => panic!("expected Xor in build-or-silent, got {other:?}"),
    };
}
