---
requires: scratch/ files to promote
produces: wave/ files
---
Route files from scratch/ to wave/.

## Destination

**wave/<wave>/** — Work items. Things to build, fix, or investigate.
```
wave/
  lfflow/
    dynamic-budgets.md
    auth-redesign.md
```

Reference material and architectural decisions belong in the wave README or the relevant milestone doc — not in a separate directory.

## Determining the wave name

1. Use explicit `--wave` flag if provided
2. Use wave name from wave configuration (if running as a wave)
3. Fall back to current worktree/branch name

**Examples:**
```
--wave lfflow + item → wave/lfflow/<slug>.md
(no flag, worktree=loopflow.lfflow) + item → wave/lfflow/<slug>.md
```

## Workflow

1. Read everything in scratch/
2. For each file worth promoting:
   - Determine destination path under `wave/<wave>/`
   - Move content (don't just copy—remove from scratch/)
3. Skip temporary analysis files that shouldn't persist

## What to promote

- Proposals with clear scope
- Follow-up work items from this diff
- Bugs or issues discovered during work
- Research or decisions that inform future wave work (fold into README or milestone doc)

## Skip entirely

- Working notes that informed decisions already captured elsewhere
- Intermediate analysis superseded by synthesis
- Branch-specific design docs (cleared on merge anyway)

## Validation

- Every promoted file must have a clear destination
- If destination already exists, merge or fail (don't silently overwrite)
- Actionable items must have clear next steps
