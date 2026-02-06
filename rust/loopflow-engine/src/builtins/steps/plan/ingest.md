---
requires: roadmap/<wave>/ items
produces: scratch/<slug>.md
---
Pick the highest-priority item from the wave's backlog and move it to scratch/.

## Wave context

**Finding the wave name:**
1. Check for `<lf:wave name="...">` tag in the prompt — this is the authoritative source
2. If no tag, look at the branch name pattern: `<wave>.main` indicates wave `<wave>`
3. If still unclear, check `roadmap/` for subdirectories — each subdirectory is a wave

The wave's roadmap (`roadmap/<wave>/`) should be included in docs. If you can't find the wave's roadmap, note this in `scratch/questions.md`.

## Staged roadmaps

Roadmaps may use numbered prefixes to indicate stages:

```
roadmap/rust/
  README.md          # Strategic context (not a pickable item)
  01-protocol.md     # Stage 1 items
  02-core-engine.md  # Stage 2 items
```

**Stage ordering rules:**
- Pick from the lowest-numbered stage first (01-* before 02-*)
- Only move to the next stage when the current stage is complete
- README.md provides principles and success criteria—use it to evaluate priority, but don't pick it

**Using README.md:**
- Follow the wave's principles when evaluating items
- Check success criteria to understand what "done" means
- Respect non-goals—don't pick items that conflict

## Selection criteria

Within a stage, evaluate each item:

**Urgency.** Is something blocked on this? Is there a deadline?

**Importance.** How much does this move the wave's success criteria forward?

**Readiness.** Are prerequisites met? Is scope clear enough to start?

**Fit.** Does it match the current area or direction?

If multiple items score similarly, prefer smaller scope—ship something.

## Workflow

1. Get wave name from `<lf:wave>` in context
2. Find `roadmap/<wave>/` in the docs
3. Read README.md for strategic context (principles, success criteria)
4. Identify the current stage (lowest numbered prefix with items)
5. Pick the highest-priority item from that stage
6. Move it to `scratch/<wave>-<slug>.md`

## Output

The selected item is moved to `scratch/<wave>-<slug>.md`. The original is removed from `roadmap/<wave>/`.

**If the wave backlog is empty:** Signal completion by writing nothing. This is not an error—it means the wave's work is done. When used in a `loop_until_empty` flow, this signals the loop should terminate.

**If items exist but none are ready:** Write `scratch/questions.md` explaining what's blocking progress.
