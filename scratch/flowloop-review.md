# Flowloop v1a review

## What was implemented

Added `lf task <linear-item-id> --wave <wave>` as the v1a task flowloop. It resolves a Linear roadmap item, creates a fresh worker worktree through the existing run-placement path, runs bounded `task-pass` cycles, polls GitHub for a merged PR, then comments the PR link on the Linear item and marks it done.

The branch also adds the `task-pass` builtin flow (`task_clarify -> task_pursue -> task_mutate`), wires `--max-turns` through normal `lf` runs, blocks `lf task` from the wave exec escape hatch, and documents the command in `README.md` and `docs/lf.md`.

Gate polish changed two runtime edges:

- Clean open PRs wait without consuming a task pass; the wall-clock cap remains the limit while waiting for a human merge.
- Task closeout now uses a PM completion helper that comments the PR and marks done without rewriting the Linear title or description.

## Key choices

The runner lives under `flowloop/task.rs` instead of inside `run_mind` or the generic flow loop. That keeps v1a small: flow passes are agentic, while termination stays a deterministic GitHub oracle.

The task oracle is GitHub PR state only. Linear completion is bookkeeping after GitHub says `MERGED`.

`-b` remains batch/headless, not budget. Hard bounds are explicit: max passes, per-pass timeout, and total wall-clock timeout. `--max-turns` is now wired through as the soft agent turn cap.

## How it fits together

`lf task` resolves the wave and roadmap item, creates a normal run-scoped worktree, stores the task prompt on the run, then loops:

```
poll PR -> maybe wait/finish -> run lf -b task-pass -> poll PR
```

The three builtin task steps carry the tier-specific judgment. The Rust runner only owns placement, bounds, polling, PM closeout, and run status updates.

## Risks and bottlenecks

This is happy-path v1a. It does not yet classify CI failures, review feedback, flaky checks, or non-convergence beyond pass and wall-clock caps.

`lf task` requires a registered wave and a PM project with an accessible Linear credential. Without those, it fails before creating the task worktree.

Polling depends on `gh pr view` in the worker worktree. If GitHub CLI auth or branch association is missing, the task continues until caps fire.

## What's not included

No project tier, wave-tier conversion, budget accounting, prod verification oracle, or automatic CI fix loop. Those stay in the follow-on flowloop work.

## Validation

- `cargo fmt`
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test -p loopflow flowloop::task --no-fail-fast`
- `cargo test -p loopflow derived_tables_cover_commands_flags_and_aliases --no-fail-fast`
- `cargo test -p loopflow load_step_finds_all_builtins --no-fail-fast`
- `cargo test -p loopflow load_flow_finds_all_builtins --no-fail-fast`
- `uv run python scripts/test.py --list`
- `uv run python scripts/test.py`

Changed-aware validation ran Rust and website suites. Rust ran `cargo nextest run --all`: 1,243 passed, 3 skipped. Website ran `cd website && uv run python dev.py test`: 61 passed, 3 skipped.
