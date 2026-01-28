# Gate Review

## What was implemented
- Added Rust workspace with `lf-core` (flow parsing/runtime, prompt handling, git/worktree helpers, store traits) and `lfd` (daemon scaffolding: gRPC server, scheduler, SQLite store, HTTP endpoints, telemetry).
- Updated wave/branch handling: timestamped branch suffixes, wave inference from lfd DB/worktree pattern, and `lfops next` now stacks branches in-place.
- Added CLI polish: step listing, flow handling improvements, and engine banner for flow runs.
- Documented daemon protocol/service work in scratch and aligned docs for `lfops next` behavior.

## Key choices
- **Stacked branches over fresh worktrees:** `lfops next` now keeps the same worktree path and creates a new branch from current HEAD to preserve local state.
- **Wave discovery priority:** explicit → lfd DB lookup → worktree pattern → roadmap inference to reduce false positives while supporting daemon-managed worktrees.
- **Timestamped branch names:** improves uniqueness and traceability without relying solely on word pairs.

## How it fits together
Rust `lf-core` provides the flow/runtime building blocks, while `lfd` wraps scheduling, storage, and RPC/HTTP surfaces around those primitives. The Python CLI continues to orchestrate user-facing workflows, with updated naming and wave discovery to integrate with daemon-managed worktrees.

## Risks and bottlenecks
- **Branch stacking workflow:** `lfops next` no longer pushes the new branch by default; if downstream tooling assumes a remote branch exists immediately, it may need a manual push.
- **Worktree detection:** worktree name heuristics (`<repo>.<wave>.main`) could misclassify custom directory names if they follow the same pattern.
- **Large Rust diff:** new crates are substantial; reviewers should focus on core runtime correctness and scheduler/store boundaries.

## What's not included
- Full daemon production hardening (auth, cluster, container execution, Postgres backend) remains out of scope.
- No CLI changes to expose new Rust daemon features beyond internal wiring and naming updates.
