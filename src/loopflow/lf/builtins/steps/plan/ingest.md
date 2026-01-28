---
requires: roadmap/<wave>/ items
produces: scratch/<slug>.md
---
Pick the highest-priority item from the wave's backlog and move it to scratch/.

## Workflow

1. Identify the wave:
   - Use explicit `--wave` flag if provided
   - Use wave name from wave configuration (if running as a wave)
   - Fall back to current worktree/branch name
2. Read items from `roadmap/<wave>/`
3. Evaluate each item: urgency, importance, dependencies, readiness
4. Pick the one that should be built next
5. Move it to `scratch/<slug>.md`

## Selection criteria

**Urgency.** Is something blocked on this? Is there a deadline?

**Importance.** How much does this move the product forward?

**Readiness.** Are prerequisites met? Is scope clear enough to start?

**Fit.** Does it match the current area or direction?

If multiple items score similarly, prefer smaller scope—ship something.

## Output

The selected item is moved to `scratch/<slug>.md`. The original is removed from `roadmap/<wave>/`.

**If the wave backlog is empty:** Signal completion by writing nothing. This is not an error—it means the wave's work is done. When used in a `loop_until_empty` flow, this signals the loop should terminate.

**If items exist but none are ready:** Write `scratch/questions.md` explaining what's blocking progress.
