# Parallel Pipeline Step Execution

Execute parallel steps in pipelines concurrently using temporary worktrees.

**Status: Implemented**

## Context

The glade-motif branch added:
- Pipeline YAML format with `parallel: [...]` steps
- `resolve_pipeline()` that marks parallel groups in resolved steps
- Per-step config (model, voice, context)

But `run_pipeline_def()` ignored `parallel_group`—it ran everything sequentially.

## What was built

This works now:

```yaml
# .lf/pipelines/ship.yaml
steps:
  - implement
  - parallel:
      - task: test
      - task: lint
  - commit
```

`implement` runs first. Then `test` and `lint` run concurrently. When both complete, `commit` runs.

## Why worktrees?

Parallel steps in the same worktree cause git conflicts. Two agents writing to the same files corrupts the working directory.

Solution: each parallel branch runs in its own temporary worktree, forked from the current state.

## Execution model

For a parallel group `[test, lint]`:

1. **Checkpoint**: Record current commit SHA
2. **Fork**: Create temp worktrees `<branch>-test-<uuid>`, `<branch>-lint-<uuid>`
3. **Run**: Execute steps concurrently via `subprocess.Popen`
4. **Wait**: Collect exit codes from all processes
5. **Merge** (if needed): For verification tasks (test, lint), no merge—just check they passed. For tasks that produce changes, merge back to main worktree.
6. **Cleanup**: Remove temp worktrees

## Implementation

Files modified:

- `src/loopflow/pipeline.py`: Added `_run_parallel_group()`, `_count_logical_steps()`, updated `run_pipeline_def()`
- `src/loopflow/lfd/collector.py`: Added `--prefix` flag for prefixed output lines
- `tests/test_pipelines.py`: Added tests for logical step counting

Key implementation details:

1. **Temporary worktrees**: Each parallel step runs in `_parallel-{task}-{uuid}` worktree
2. **Prefixed output**: Lines show `[task] output...` so interleaved output is readable
3. **Cleanup always**: Temp worktrees removed even on failure
4. **No autocommit/push**: Parallel steps are verification-only (test, lint)

## Scope decisions

### Verification vs. mutation

Most parallel steps are verification (test, lint, type-check). They don't produce changes—they just check the current state passes.

For these, no merge is needed. Just run in parallel worktrees and check exit codes.

Mutation steps (parallel implementations) are trickier—merging concurrent changes is complex. Deferred.

**This iteration**: parallel verification steps only. No merge back.

### Cleanup

Always clean up temp worktrees, even on failure. Debug info goes in logs, not worktree directories.

### Output handling

Used option 1: prefix each line with `[task]`. Shows progress and source of each line.

## YAML examples

```yaml
# Parallel verification
steps:
  - implement
  - parallel:
      - test
      - lint
  - commit

# Parallel with per-step config
steps:
  - implement
  - parallel:
      - task: test
        config:
          context: [tests/]
      - task: lint
  - commit

# Model comparison (different models in parallel)
steps:
  - parallel:
      - task: implement
        config:
          model: claude:opus
      - task: implement
        config:
          model: codex:o3
  # Manual comparison step after
```

## Future work

- Mutation steps: parallel implementations that merge back
- Model racing: run same task with different models, pick winner
