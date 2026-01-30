# Design Review: lfd stacking commands (next, rebase)

## What was implemented

This branch adds stacking workflow commands to `lfd` for managing dependent PRs:

1. **`lfd next`** - Stack a new branch on top of current work
   - Enables auto-merge on current PR
   - Creates new branch from HEAD with timestamped name
   - Records base tracking (base_branch, base_commit) for squash-aware rebase

2. **`lfd rebase`** - Rebase stacked branch after base PR lands
   - Detects if base PR was squash-merged
   - Uses `--onto` rebase with recorded base_commit for clean replay
   - Clears stacking info after successful rebase

3. **Database migration** - Adds `base_branch` and `base_commit` columns to waves table

4. **Wave model updates** - New fields and `update_wave_stacking()` persistence function

## Key choices

**Squash-merge awareness:** Standard git stacking breaks when base PRs are squash-merged because the original commits disappear. Recording `base_commit` at stack time enables `git rebase --onto origin/main <base_commit>`, which replays only the stacked commits without trying to replay the (now-squashed) base commits.

**Wave-centric design:** Commands resolve the current wave from the worktree path via `get_wave_by_worktree()`. This integrates with the existing wave model rather than creating standalone git tooling.

**Auto-merge integration:** `lfd next` enables GitHub auto-merge on the current PR before stacking. This is opinionated—the assumption is that if you're stacking, you're ready to land the base PR when CI passes.

**Inferred wave resolution:** Both commands accept an optional wave name argument but default to inferring from the current worktree. This matches the ergonomic pattern of other `lfd` commands.

## How it fits together

```
User runs: lfd next
       │
       ▼
Resolve wave from worktree or name argument
       │
       ▼
Get/create PR, enable auto-merge
       │
       ▼
Record current branch as base_branch
Record HEAD SHA as base_commit
       │
       ▼
Create new branch: {wave}.{timestamp}.{word-pair}
Update wave.branch, wave.base_branch, wave.base_commit
       │
       ▼
User works on stacked changes...
       │
       ▼
Base PR lands (squash-merged to main)
       │
       ▼
User runs: lfd rebase
       │
       ▼
Fetch origin, check base PR state
       │
       ▼
git rebase --onto origin/main {base_commit}
(Replays only commits after base_commit)
       │
       ▼
Force-push, clear base_branch/base_commit
```

## Risks and bottlenecks

**GitHub API dependency:** Commands shell to `gh` CLI for PR operations. If `gh` isn't installed or configured, auto-merge and PR state checks fail (with warnings, not hard failures).

**Rebase conflicts:** Squash-aware rebase doesn't eliminate conflicts—it just avoids false conflicts from already-merged commits. Users still need to resolve real conflicts manually.

**Single-level stacking:** The current design tracks one base branch per wave. Multi-level stacking (A → B → C) would require a different data model or chained rebases.

**Force-push safety:** Uses `--force-with-lease` which is safe against overwriting others' work, but still requires care in shared-branch scenarios.

## What's not included

- Multi-level stacking support (only single-level A → B)
- Automatic conflict resolution
- Integration with `lfd loop` or other autonomous modes
- Visualization of stacking relationships in `lfd status`
- Automatic rebase when base PR lands (requires webhook or polling)
