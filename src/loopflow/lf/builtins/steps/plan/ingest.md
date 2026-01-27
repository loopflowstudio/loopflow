---
requires: roadmap/ items
produces: scratch/<slug>.md
---
Pick the highest-priority roadmap item and move it to scratch/.

## Workflow

1. Read all roadmap items under `roadmap/**/*.md`
2. Evaluate each: urgency, importance, dependencies, readiness
3. Pick the one that should be built next
4. Move it from `roadmap/` to `scratch/`

## Selection criteria

**Urgency.** Is something blocked on this? Is there a deadline?

**Importance.** How much does this move the product forward?

**Readiness.** Are prerequisites met? Is scope clear enough to start?

**Fit.** Does it match the current area or direction?

If multiple items score similarly, prefer smaller scope—ship something.

## Output

The selected roadmap item is moved to `scratch/<slug>.md`. The original is removed from `roadmap/`.

If no roadmap items exist or none are ready, write `scratch/questions.md` explaining what's missing.
