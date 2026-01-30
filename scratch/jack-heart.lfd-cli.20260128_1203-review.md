# Review: lf flow output and worktree robustness

## What was implemented

1. **Flow execution overview** — `lf flow` now prints a header showing the flow name, model, step count, and compact outline (e.g., `review → fork(reduce×3) → publish`) before execution.

2. **Improved step output** — Step headers use bold formatting and show model/direction/context config inline. Token profiles now print before the prompt is finalized, with consistent structure between regular and interactive steps.

3. **Orphaned branch handling** — `worktrees.create()` now detects and deletes orphaned local branches (branches that exist without a worktree) before creating a new worktree. `create_with_schema()` provides an explicit error message when an orphaned branch is detected.

4. **`lfd reset` command** — Stops all running waves, deletes the database, and reinitializes with the latest schema. Useful for development and recovery from corrupt state.

5. **Wave name resolution** — Commands `lfd status`, `lfd stop`, `lfd prs`, `lfd rm`, and `lfd logs` now accept wave names in addition to IDs.

## Key choices

**Flow outline format** — Used `→` arrows and compact notation like `fork(step×3)` for quick scannability. Shows first 3 options for `choose` steps to avoid clutter.

**Orphan branch deletion in `create()`** — Auto-deletes orphaned branches silently. This matches the function's existing behavior of reusing worktrees when they exist. In contrast, `create_with_schema()` raises an explicit error, since schema-based creation is more deliberate.

**Import ordering** — Fixed `reset_db` import to maintain alphabetical order within the `loopflow.lfd.*` block.

## How it fits together

The flow output changes are confined to `flow.py` formatting functions. The worktree changes add two private helpers (`_local_branch_exists`, `_delete_local_branch`) and integrate them into the creation path. The `lfd reset` command uses existing `stop_wave()` and the new `reset_db()` function from `db.py`.

## Risks and bottlenecks

- **Silent branch deletion** — If a user manually created a branch with the same name as a worktree, `create()` will delete it without warning. This matches the function's existing reuse behavior but could surprise users.

- **`lfd reset` is destructive** — By design, but requires `-f` for scripted use.

## What's not included

- No changes to daemon execution output (only CLI `lf flow` output)
- No changes to flow parsing or DAG logic
- No test coverage for the new `_local_branch_exists`/`_delete_local_branch` helpers (they're thin wrappers over git commands)
