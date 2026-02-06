# loopflow-ops

```rust
use loopflow_ops::{commit_workflow, CommitOptions, NullProgress};

let options = CommitOptions {
    add: true,
    lint: true,
    push: true,
    create_draft_pr: true,
    task: "commit".to_string(),
    flow_parents: Vec::new(),
    message: None,
};

commit_workflow(repo_path, &options, &NullProgress)?;
```

```rust
use loopflow_ops::{create_or_update_pr, PrOptions, NullProgress};

let result = create_or_update_pr(
    repo_path,
    &PrOptions { refresh: false, lint: true },
    &NullProgress,
)?;
println!("PR: {}", result.url);
```

Orchestrates high-level `lf ops` workflows on top of `loopflow-engine` primitives.

- `commit_workflow` stages, lints, generates a message, commits, and optionally pushes.
- `create_or_update_pr` keeps PRs fresh and can open existing ones.
- `land`, `next_branch`, and `abandon_branch` combine git + GitHub CLI steps into one call.

Use `Progress` to surface status updates or confirm destructive actions.
