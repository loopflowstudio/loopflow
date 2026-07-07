## Try it!

```bash
lf task <linear-item-id> --wave designer
lf task <linear-item-id> --wave designer --max-passes 4 --wall-clock-secs 3600 --max-turns 20
```

The command resolves the Linear item from the wave roadmap, creates a run-scoped worker worktree, runs bounded `task-pass` cycles, waits for the PR to merge, then comments the PR link on the Linear item and marks it done.

Project flowloops are built as a library tier for the next wave-spawn slice. They read kr-labeled Linear items, run `project-pass` while any KR is open, and refuse to start when the KR set is empty.

Validation:

```bash
git diff --check main...HEAD
cargo fmt --check
cargo clippy -p loopflow -- -D warnings
cargo test -p loopflow flowloop --no-fail-fast
cargo test -p loopflow derived_tables_cover_commands_flags_and_aliases --no-fail-fast
cargo run -q -p loopflow --bin lf -- task fake --max-turns 20 --help
uv run python scripts/test.py --list
```

Changed-aware validation from `scripts/test.py --list` selects Rust and website suites for this branch.

## Intent

This turns flowloop from a one-off task runner into a shared tier substrate. Tasks and projects now use the same pass runner, run lifecycle, and deterministic oracle pattern: the agent chooses moves inside each pass, while GitHub or Linear decides whether the loop is done.

## Assumptions

Waves are registered in the local run registry before task/project flowloops start. The PM provider is Linear, roadmap items expose labels, and GitHub CLI plus Linear auth are available locally. Human-gated merge remains the default for `lf task`: the runner exits only after GitHub reports the PR merged.

## Key decisions

Tier behavior lives in builtin step text, not Rust branching. Rust binds each tier to a pass flow and oracle, then enforces bounds and records state.

The project KR oracle uses a `kr` Linear label for this slice. Empty KR sets fail fast because "no KRs" is an unclear project, not a completed one.

Clean open task PRs wait without consuming another pass. Waiting for a human merge should not look like failed agent progress.

## Not included

This does not replace the live wave resident turn with `wave-pass`, rename the `MindState` wire surface, add `lf project`, or implement CI/review repair loops. Those are follow-on slices now that the shared runtime exists.
