---
requires: polished code on branch
produces: extended functionality
---
Extend what this branch does. Push beyond "done" to "great."

## Goal

The branch works and it's clean. Now: what would make it great instead of just done? What adjacent features become easy? What quality upgrades are now obvious?

This is exploratory—propose ideas, implement the best one. The human can reject or redirect.

## Workflow

1. **Review what's there**
   The diff shows working, polished code. Understand what it does well.

2. **Identify extensions**
   Ask:
   - What natural next step does this enable?
   - What would users wish it also did?
   - What quality upgrade is now obvious (speed, error messages, API clarity)?
   - What nearby debt could be paid while context is fresh?

3. **Pick one**
   Highest impact. Don't scatter effort across multiple extensions.

4. **Implement it**
   Tests required. This isn't a prototype.

## What to extend

**Natural extensions.** The branch adds auth—what about password reset? The branch adds caching—what about cache invalidation UI?

**Quality upgrades.** It works—could it be fast? Could errors be clearer? Could the API be more intuitive?

**Debt paydown.** Code nearby that's been annoying. Patterns that should match the new code.

## Scope

**One thing.** Pick the highest-impact extension and do it well.

**Stay coherent.** The expansion should feel like it belongs with the original branch. Unrelated improvements belong in a different branch.

**Tests required.** New behavior needs tests.
