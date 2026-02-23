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
    direction: [designer, product-engineer]
"#,
    );

    let flow = load_flow("sample", repo).unwrap();
    assert_eq!(flow.name, "sample");
    assert_eq!(flow.items.len(), 2);
    assert_eq!(
        flow.items[0],
        FlowItem::Step(Step {
            name: "implement".to_string(),
            model: None,
            directions: vec![],
            interactive: None,
            content: None,
        })
    );
    assert_eq!(
        flow.items[1],
        FlowItem::Step(Step {
            name: "review".to_string(),
            model: None,
            directions: vec!["designer".to_string(), "product-engineer".to_string()],
            interactive: Some(true),
            content: None,
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
- fork:
    branches:
      - step: { name: implement }
      - step: { name: polish }
- fork:
    branches:
      - step: { name: quick }
      - step: { name: deep }
- flow: nested
"#,
    );

    let flow = load_flow("forked", repo).unwrap();
    assert_eq!(flow.items.len(), 3);
    match &flow.items[0] {
        FlowItem::Fork { branches } => {
            assert_eq!(branches.len(), 2);
        }
        _ => panic!("expected fork"),
    }
    match &flow.items[1] {
        FlowItem::Fork { branches } => {
            assert_eq!(branches.len(), 2);
        }
        _ => panic!("expected fork"),
    }
    match &flow.items[2] {
        FlowItem::FlowRef(name) => {
            assert_eq!(name, "nested");
        }
        _ => panic!("expected flow ref"),
    }
}

#[test]
fn fork_select_is_rejected() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "forked",
        r#"
- fork:
    branches:
      - step: { name: implement }
    select: all
"#,
    );

    let err = load_flow("forked", repo).expect_err("fork select should fail");
    let message = err.to_string();
    assert!(message.contains("fork select modes are not supported"));
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
/// expanded as sub-flows, not treated as step names. This was the root
/// cause of `step not found: publish` in wave-reduce.
#[test]
fn expand_flow_resolves_plain_string_as_subflow() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    // "publish" is a flow containing two steps
    write_flow(repo, "publish", "- step-a\n- step-b");
    // "parent" references "publish" as a plain string (not `flow: publish`)
    write_flow(repo, "parent", "- review\n- publish");

    let flow = load_flow("parent", repo).unwrap();
    let items = expand_flow(&flow, repo).unwrap();

    // Should expand to 3 steps: review, step-a, step-b
    assert_eq!(items.len(), 3, "publish should expand into its sub-steps");
    match &items[0] {
        ConcreteItem::Step(s) => assert_eq!(s.step.name, "review"),
        _ => panic!("expected step"),
    }
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

    // "review" exists as both a step and as a flow (auto-wrapped)
    write_step(repo, "review", "Review the code.");
    write_flow(repo, "parent", "- review");

    let flow = load_flow("parent", repo).unwrap();
    let items = expand_flow(&flow, repo).unwrap();

    // Should keep as a single step (not expand into a sub-flow)
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
/// fork(reduce×3), consolidate, add-to-wave.
#[test]
fn builtin_wave_reduce_expands_publish_subflow() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let flow = load_flow("wave-reduce", repo).unwrap();
    let items = expand_flow(&flow, repo).unwrap();

    // fork + consolidate + add-to-wave = 3
    assert_eq!(
        items.len(),
        3,
        "wave-reduce should expand publish into consolidate + add-to-wave"
    );

    // Step 0: fork (reduce × 3 directions)
    assert!(
        matches!(&items[0], ConcreteItem::Fork(_)),
        "expected fork at index 0"
    );
    // Step 1: consolidate (from publish sub-flow)
    match &items[1] {
        ConcreteItem::Step(s) => assert_eq!(s.step.name, "consolidate"),
        _ => panic!("expected consolidate step"),
    }
    // Step 2: add-to-wave (from publish sub-flow)
    match &items[2] {
        ConcreteItem::Step(s) => assert_eq!(s.step.name, "add-to-wave"),
        _ => panic!("expected add-to-wave step"),
    }
}
