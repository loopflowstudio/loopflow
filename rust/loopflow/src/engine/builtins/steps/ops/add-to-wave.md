---
requires: scratch/ files to promote
produces: roadmap/ or reports/ files
---
Route files from scratch/ to roadmap/ or reports/ based on content type.

## Two destinations

**roadmap/<wave>/** — Actionable work items. Things to build next.
```
roadmap/
  lfflow/
    dynamic-budgets.md
    auth-redesign.md
```

**reports/** — Reference material. Context for understanding, not immediately actionable.
```
reports/
  landscape.md
  target-customer.md
```

## Routing logic

For each file in scratch/, decide:

**Is this actionable follow-up work?**
- Has clear scope and next steps
- Someone should build this
- → `roadmap/<wave>/<slug>.md`

**Is this reference/context?**
- Research with lasting value
- Decisions that should be remembered
- Context for future work
- → `reports/<topic>.md`

## Determining the wave name

For actionable items going to `roadmap/`:

1. Use explicit `--wave` flag if provided
2. Use wave name from wave configuration (if running as a wave)
3. Fall back to current worktree/branch name

**Examples:**
```
--wave lfflow + actionable item → roadmap/lfflow/<slug>.md
(no flag, worktree=loopflow.lfflow) + actionable item → roadmap/lfflow/<slug>.md
reference material → reports/<topic>.md
```

## Workflow

1. Read everything in scratch/
2. For each file worth promoting:
   - Decide: actionable (roadmap/) or reference (reports/)
   - Determine destination path
   - Move content (don't just copy—remove from scratch/)
3. Skip temporary analysis files that shouldn't persist

## What to promote

**To roadmap/ (actionable):**
- Proposals with clear scope
- Follow-up work items from this diff
- Bugs or issues discovered during work

**To reports/ (reference):**
- Research with lasting value
- Architectural decisions
- Analysis that informs future work

**Skip entirely:**
- Working notes that informed decisions
- Intermediate analysis superseded by synthesis
- Branch-specific design docs (cleared on merge anyway)

## Validation

- Every promoted file must have a clear destination
- If destination already exists, merge or fail (don't silently overwrite)
- Actionable items must have clear next steps
