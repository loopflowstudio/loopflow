use std::fs;
use std::path::Path;

use loopflow_engine::flow::{FlowItem, Step};
use loopflow_engine::load_flow;
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
    synthesize: consolidate
- choose:
    prompt: Pick a path
    options:
      fast:
        - step: { name: quick }
      thorough:
        - step: { name: deep }
- loop_until_empty:
    steps:
      - step: { name: iterate }
"#,
    );

    let flow = load_flow("forked", repo).unwrap();
    assert_eq!(flow.items.len(), 3);
    match &flow.items[0] {
        FlowItem::Fork {
            branches,
            synthesize,
        } => {
            assert_eq!(branches.len(), 2);
            assert_eq!(synthesize.as_deref(), Some("consolidate"));
        }
        _ => panic!("expected fork"),
    }
    match &flow.items[1] {
        FlowItem::Choose { prompt, options } => {
            assert_eq!(prompt, "Pick a path");
            assert!(options.contains_key("fast"));
            assert!(options.contains_key("thorough"));
        }
        _ => panic!("expected choose"),
    }
    match &flow.items[2] {
        FlowItem::LoopUntilEmpty {
            steps,
            wave,
            max_iterations,
        } => {
            assert_eq!(steps.len(), 1);
            assert!(wave.is_none());
            assert_eq!(*max_iterations, 100);
            assert_eq!(
                steps[0],
                FlowItem::Step(Step {
                    name: "iterate".to_string(),
                    model: None,
                    directions: vec![],
                    interactive: None,
                    content: None,
                })
            );
        }
        _ => panic!("expected loop"),
    }
}
