---
requires: diff vs main
produces: ambitious improvements
---
Explore ambitious changes that extend what this branch is already doing.

## Goal

Push beyond the immediate scope. If the branch adds feature X, what would make X great instead of just done? What adjacent features become easy now? What technical debt could be paid down while the context is fresh?

This is exploratory—propose ideas, implement the best one. The human can reject or redirect.

## What to explore

**Natural extensions.** The branch adds auth—what about password reset? The branch adds caching—what about cache invalidation UI?

**Quality upgrades.** The branch works—could it be fast? Could errors be more helpful? Could the API be more intuitive?

**Debt paydown.** Code nearby that's been annoying. Patterns that should be updated to match the new code.

## Constraints

**One thing.** Pick the highest-impact extension and do it well. Don't scatter effort.

**Stay coherent.** The expansion should feel like it belongs with the original branch work. If it's unrelated, it belongs in a different branch.

**Tests required.** New behavior needs tests. This isn't a prototype.
