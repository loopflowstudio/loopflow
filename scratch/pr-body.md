## Try it!

```bash
lf task <linear-item-id> --wave designer
lf task <linear-item-id> --wave designer --max-passes 4 --wall-clock-secs 3600 --max-turns 20
```

The command resolves the Linear item from the wave roadmap, creates a run-scoped worker worktree, runs bounded `task-pass` cycles, waits for the PR to merge, then comments the PR link on the Linear item and marks it done.

Validation:

```bash
cargo fmt --check
cargo clippy -- -D warnings
uv run python scripts/test.py
```

Changed-aware validation ran Rust and website suites. Rust: `cargo nextest run --all` with 1,243 passed and 3 skipped. Website: 61 passed and 3 skipped.

## Intent

This lands v1a of the task flowloop: one unattended command for turning a Linear roadmap item into a small PR lifecycle, with deterministic termination based on GitHub merge state instead of an agent claiming completion.

## Assumptions

The wave is already registered in the local run registry and has PM frontmatter pointing at a Linear project. GitHub CLI auth and Linear PM auth are available. Human-gated merge remains the default: `task_mutate` submits the PR, and the runner exits only after GitHub reports it merged.

## Key decisions

The task flowloop is a thin Rust runner outside the existing wave mind and outside the generic flow loop. Each pass is just `lf -b task-pass`, while Rust owns worktree placement, bounds, PR polling, run status, and Linear closeout.

Bounds are explicit because `-b` means batch, not budget: max passes, per-pass timeout, wall-clock timeout, and optional `--max-turns`.

Clean open PRs wait without consuming a pass. That keeps "waiting for merge" separate from "try another agent pass."

## Not included

No project tier, wave-tier conversion, prod verification, budget accounting, or CI/review fix loop. v1a escalates on caps; smarter repair behavior is later flowloop hardening.
