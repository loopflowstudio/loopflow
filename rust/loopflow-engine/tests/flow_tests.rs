use std::fs;
use std::path::Path;

use loopflow_engine::flow::{ConcreteItem, FlowItem, ForkSelect, Step};
use loopflow_engine::{expand_flow, load_flow};
use tempfile::TempDir;

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
    select: all
    synthesize: consolidate
- fork:
    branches:
      - step: { name: quick }
      - step: { name: deep }
    select: prompt
    prompt: Pick a path
- flow: nested
"#,
    );

    let flow = load_flow("forked", repo).unwrap();
    assert_eq!(flow.items.len(), 3);
    match &flow.items[0] {
        FlowItem::Fork {
            branches,
            select,
            synthesize,
        } => {
            assert_eq!(branches.len(), 2);
            assert_eq!(select, &ForkSelect::All);
            assert_eq!(synthesize.as_deref(), Some("consolidate"));
        }
        _ => panic!("expected fork"),
    }
    match &flow.items[1] {
        FlowItem::Fork { select, .. } => {
            assert_eq!(
                select,
                &ForkSelect::Prompt {
                    prompt: "Pick a path".to_string()
                }
            );
        }
        _ => panic!("expected prompt fork"),
    }
    match &flow.items[2] {
        FlowItem::FlowRef(name) => {
            assert_eq!(name, "nested");
        }
        _ => panic!("expected flow ref"),
    }
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
