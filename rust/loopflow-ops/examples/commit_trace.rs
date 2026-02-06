//! Example binary for commit trace parity testing.

use loopflow_ops::{commit_workflow_traced, CommitOptions};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut add = true;
    let mut lint = true;
    let mut push = false;

    for arg in &args[1..] {
        if let Some(value) = arg.strip_prefix("--add=") {
            add = value.parse().unwrap_or(true);
        } else if let Some(value) = arg.strip_prefix("--lint=") {
            lint = value.parse().unwrap_or(true);
        } else if let Some(value) = arg.strip_prefix("--push=") {
            push = value.parse().unwrap_or(false);
        }
    }

    let options = CommitOptions {
        add,
        lint,
        push,
        create_draft_pr: false,
        task: "test".to_string(),
        flow_parents: vec![],
        message: None,
    };

    let trace = commit_workflow_traced(&options);
    println!("{}", trace);
}
