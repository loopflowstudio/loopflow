# Prune

## Vision

Aggressive worktree cleanup. Deterministic Rust logic catches the easy cases (merged, remote-gone, empty). An agent step acts as the backstop — investigates what happened to each remaining worktree and removes the ones that are clearly done.

Not a safety tool. Dirty worktrees are never auto-removed — they get flagged for human attention.

## Strategy

Two layers, sequenced so the agent has less to do over time:

1. **Deterministic Rust prune** (`wt prune`, `wt list`). Extend `WorktreeState` with `dirty` and `remote_gone` fields. Surface `landed-dirty` and `remote-gone` as first-class states in `wt list`. Make `wt prune --force` remove remote-gone clean worktrees. As more signals land in Rust, the agent step shrinks.

2. **Agent step** (`.lf/steps/prune.md`). Runs after deterministic prune. Checks PRs, commit history, and main for evidence that remaining worktrees are done. Removes the obvious ones, flags the rest.

### Key files

- `rust/loopflow/src/engine/worktrees.rs` — `WorktreeState`, `list_worktrees()`
- `rust/loopflow/src/lf/commands/ops/mod.rs` — `wt_list()`, `wt_prune()`, `wt_remove()`
- `.lf/steps/prune.md` — agent step (new)

### Invariants

- Dirty worktrees are NEVER auto-removed — by Rust code or agent. They get flagged.
- The agent never deletes the current worktree or main.
- Remote-gone detection requires network (`ls-remote`). Skip gracefully offline.
- The step works from any worktree (resolves main repo root).

## Goals

- `wt list` surfaces `landed-dirty` and `remote-gone` as distinct states
- `wt prune --force` removes remote-gone clean worktrees alongside merged ones
- `lf prune` runs the agent backstop — investigates and removes clearly-done worktrees
- Over time, more signals move from agent to Rust

## Risks

- **False positive prune.** Mitigated by never touching dirty worktrees and requiring evidence (PR merged, commits in main) before removal.
- **Network dependency.** `ls-remote` can be slow or unavailable. The agent and Rust code must degrade gracefully.
- **Squash-merge detection.** GitHub squash merges rewrite SHAs — commit-based checks won't find the original SHAs in main. The agent checks PR state instead.

## Metrics

- Number of worktrees pruned per run (deterministic vs agent-assisted breakdown)
- False positive rate: worktrees removed that shouldn't have been (target: 0)
- % of done worktrees caught by deterministic Rust logic vs requiring agent (target: increasing over time)
- Orphan worktree accumulation: count of stale worktrees >7 days old (target: 0)
