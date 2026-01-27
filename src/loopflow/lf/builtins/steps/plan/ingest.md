---
requires: roadmap/ items
produces: scratch/<slug>.md
---
Pick the highest-priority roadmap item in this area and move it to scratch/.

## Workflow

1. Identify the roadmap scope:
   - If running with an area (e.g., `src/api/`), read from `roadmap/<area>/` (e.g., `roadmap/src/api/`)
   - Otherwise, read from root `roadmap/`
2. Evaluate each item: urgency, importance, dependencies, readiness
3. Pick the one that should be built next
4. Move it to `scratch/<slug>.md`

## Selection criteria

**Urgency.** Is something blocked on this? Is there a deadline?

**Importance.** How much does this move the product forward?

**Readiness.** Are prerequisites met? Is scope clear enough to start?

**Fit.** Does it match the current area or direction?

If multiple items score similarly, prefer smaller scope—ship something.

## Output

The selected roadmap item is moved to `scratch/<slug>.md`. The original is removed from its roadmap location.

If no roadmap items exist in the relevant scope or none are ready, write `scratch/questions.md` explaining what's missing.
