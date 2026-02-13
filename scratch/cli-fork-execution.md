# CLI Fork Execution (Current State)

`lf flow` now executes `fork(select: all)` directly in CLI mode.

## Shipped behavior

- `FlowAction::Fork` no longer skips in CLI runs.
- Each fork branch gets its own sibling worktree using dotted naming (`<repo>.fork-<index>` or `<repo>.<branch>.fork-<index>`).
- Branch names follow `{current-branch}-fork-{index}`.
- CLI self-invokes fork branch steps in parallel subprocesses (`lf <step> -b -d <direction>`), with each subprocess running in its fork worktree.
- Fork metadata is written to `.lf/fork-manifest.json` in the original worktree.
- Optional synthesize runs after all fork branches complete, using manifest data.
- Cleanup runs after synthesis: manifest removal + fork worktree removal.
- Failure policy is fail-late: wait for all branches, then fail with aggregate error count.

## Why this matters

- `lf flow roadmap-reduce` and related forked plan flows are testable from CLI without requiring daemon execution.
- Fork branches stay isolated while still starting from the same parent HEAD.
- Synthesis has explicit branch/worktree metadata instead of inferring state.

## Current constraints

- CLI supports `fork.select: all` only.
- Parallel fork subprocess logs share stdio and can interleave.
- No automatic merge of fork outputs; synthesis remains agent-driven.

## Follow-ups

- Add CLI support for `fork.select: one` and `fork.select: prompt`.
- Improve branch log readability (capture and prefix per-branch output).
