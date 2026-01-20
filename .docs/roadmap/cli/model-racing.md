---
status: proposed
area: cli
created_at: 2026-01-20T15:30:00
---

# Model Racing: Run Same Task with Multiple Models

Race Claude vs Codex on the same task, pick the winner.

## The Problem

The `RaceConfig` structure exists in `flows.py` but there's no actual racing implementation. Users can't run the same prompt against multiple models in parallel and compare results.

## Proposed Solution

Add `lf race` command:

```bash
# Race two models on the same task
lf race debug -v --models claude:sonnet,codex:o3

# Race with custom judge
lf race implement --models claude:opus,claude:sonnet --judge review
```

Each model runs in its own worktree. Results are compared via `@compare` step or custom judge.

## Implementation

1. Create worktrees for each model: `feature-race-1`, `feature-race-2`
2. Run step in parallel across worktrees
3. After all complete, run judge step that sees both diffs
4. Output recommendation, let user pick winner

## Files to Change

- `src/loopflow/lf/run.py` - Add race execution logic
- `src/loopflow/lf/__init__.py` - Add `race` command
- `src/loopflow/lf/worktrees.py` - Support race worktree creation

## Open Questions

1. How to handle races where one model fails? Continue with survivors?
2. Should races support >2 models? Gets expensive quickly.
3. Auto-cleanup losing worktrees, or let user review first?
